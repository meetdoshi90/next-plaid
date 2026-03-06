//! Application state management for the next_plaid API.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use arc_swap::{ArcSwap, Guard};
use next_plaid::MmapIndex;
use parking_lot::RwLock;

use crate::error::{ApiError, ApiResult};
use crate::models::IndexConfigStored;

static LOADING_LOCKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<std::sync::Mutex<()>>>>> =
    OnceLock::new();

pub struct IndexSlot {
    active: ArcSwap<MmapIndex>,
}

impl IndexSlot {
    pub fn new(index: MmapIndex) -> Self {
        Self {
            active: ArcSwap::from_pointee(index),
        }
    }

    pub fn load(&self) -> Guard<Arc<MmapIndex>> {
        self.active.load()
    }

    pub fn store(&self, new_index: MmapIndex) {
        self.active.store(Arc::new(new_index));
    }
}

fn get_loading_lock(name: &str) -> Arc<std::sync::Mutex<()>> {
    let locks = LOADING_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut locks_guard = locks
        .lock()
        .expect("LOADING_LOCKS mutex poisoned");
    locks_guard
        .entry(name.to_string())
        .or_insert_with(|| Arc::new(std::sync::Mutex::new(())))
        .clone()
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub index_dir: PathBuf,
    pub default_top_k: usize,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            index_dir: PathBuf::from("./indices"),
            default_top_k: 10,
        }
    }
}

pub struct AppState {
    pub config: ApiConfig,
    indices: RwLock<HashMap<String, Arc<IndexSlot>>>,
    index_configs: RwLock<HashMap<String, IndexConfigStored>>,
}

impl AppState {
    pub fn new(config: ApiConfig) -> Self {
        if !config.index_dir.exists() {
            std::fs::create_dir_all(&config.index_dir).ok();
        }
        Self {
            config,
            indices: RwLock::new(HashMap::new()),
            index_configs: RwLock::new(HashMap::new()),
        }
    }

    pub fn index_path(&self, name: &str) -> PathBuf {
        self.config.index_dir.join(name)
    }

    pub fn index_exists_on_disk(&self, name: &str) -> bool {
        self.index_path(name).join("metadata.json").exists()
    }

    pub fn load_index(&self, name: &str) -> ApiResult<()> {
        let path = self.index_path(name);
        let path_str = path.to_string_lossy().to_string();

        if !path.join("metadata.json").exists() {
            return Err(ApiError::IndexNotFound(name.to_string()));
        }

        let idx = MmapIndex::load(&path_str)?;
        let mut indices = self.indices.write();
        indices.insert(name.to_string(), Arc::new(IndexSlot::new(idx)));
        Ok(())
    }

    pub fn get_index_slot(&self, name: &str) -> ApiResult<Arc<IndexSlot>> {
        {
            let indices = self.indices.read();
            if let Some(idx) = indices.get(name) {
                return Ok(Arc::clone(idx));
            }
        }

        let loading_lock = get_loading_lock(name);
        let _guard = loading_lock.lock().unwrap();

        {
            let indices = self.indices.read();
            if let Some(idx) = indices.get(name) {
                return Ok(Arc::clone(idx));
            }
        }

        self.load_index(name)?;

        let indices = self.indices.read();
        indices
            .get(name)
            .cloned()
            .ok_or_else(|| ApiError::IndexNotFound(name.to_string()))
    }

    pub fn get_index_for_read(&self, name: &str) -> ApiResult<Guard<Arc<MmapIndex>>> {
        let slot = self.get_index_slot(name)?;
        Ok(slot.load())
    }

    pub fn register_index(&self, name: &str, index: MmapIndex) {
        let mut indices = self.indices.write();
        if let Some(slot) = indices.get(name) {
            slot.store(index);
        } else {
            indices.insert(name.to_string(), Arc::new(IndexSlot::new(index)));
        }
    }

    pub fn unload_index(&self, name: &str) -> bool {
        let mut indices = self.indices.write();
        indices.remove(name).is_some()
    }

    pub fn reload_index(&self, name: &str) -> ApiResult<()> {
        let path = self.index_path(name);
        let path_str = path.to_string_lossy().to_string();

        if !path.join("metadata.json").exists() {
            return Err(ApiError::IndexNotFound(name.to_string()));
        }

        let new_idx = MmapIndex::load(&path_str)?;

        let indices = self.indices.read();
        if let Some(slot) = indices.get(name) {
            slot.store(new_idx);
            Ok(())
        } else {
            drop(indices);
            let mut indices = self.indices.write();
            indices.insert(name.to_string(), Arc::new(IndexSlot::new(new_idx)));
            Ok(())
        }
    }

    pub fn get_index_config(&self, name: &str) -> Option<IndexConfigStored> {
        {
            let configs = self.index_configs.read();
            if let Some(config) = configs.get(name) {
                return Some(config.clone());
            }
        }

        let config_path = self.index_path(name).join("config.json");
        let config = std::fs::File::open(&config_path)
            .ok()
            .and_then(|f| serde_json::from_reader::<_, IndexConfigStored>(f).ok())?;

        {
            let mut configs = self.index_configs.write();
            configs.insert(name.to_string(), config.clone());
        }

        Some(config)
    }

    pub fn set_index_config(&self, name: &str, config: IndexConfigStored) -> ApiResult<()> {
        let config_path = self.index_path(name).join("config.json");
        let config_file = std::fs::File::create(&config_path)
            .map_err(|e| ApiError::Internal(format!("Failed to create config file: {}", e)))?;
        serde_json::to_writer_pretty(config_file, &config)
            .map_err(|e| ApiError::Internal(format!("Failed to write config: {}", e)))?;

        let mut configs = self.index_configs.write();
        configs.insert(name.to_string(), config);
        Ok(())
    }

    pub fn invalidate_config_cache(&self, name: &str) {
        let mut configs = self.index_configs.write();
        configs.remove(name);
    }

    pub fn list_all(&self) -> Vec<String> {
        let mut names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.config.index_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let path = entry.path();
                    if path.join("metadata.json").exists() || path.join("config.json").exists() {
                        if let Some(name) = entry.file_name().to_str() {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
        names.sort();
        names
    }

    pub fn loaded_count(&self) -> usize {
        self.indices.read().len()
    }

    pub fn get_all_index_summaries(&self) -> Vec<crate::models::IndexSummary> {
        self.list_all()
            .iter()
            .filter_map(|name| self.get_index_summary(name).ok())
            .collect()
    }

    pub fn get_index_summary(&self, name: &str) -> ApiResult<crate::models::IndexSummary> {
        let path = self.index_path(name);
        let path_str = path.to_string_lossy().to_string();

        let stored_config = self.get_index_config(name);
        let max_documents = stored_config.as_ref().and_then(|c| c.max_documents);
        let nbits = stored_config.as_ref().map(|c| c.nbits).unwrap_or(4);

        let metadata_path = path.join("metadata.json");
        if !metadata_path.exists() {
            return Ok(crate::models::IndexSummary {
                name: name.to_string(),
                num_documents: 0,
                num_embeddings: 0,
                num_partitions: 0,
                dimension: 0,
                nbits,
                avg_doclen: 0.0,
                has_metadata: false,
                max_documents,
            });
        }

        let metadata_file = std::fs::File::open(&metadata_path)
            .map_err(|e| ApiError::Internal(format!("Failed to open metadata: {}", e)))?;
        let metadata: serde_json::Value = serde_json::from_reader(metadata_file)
            .map_err(|e| ApiError::Internal(format!("Failed to parse metadata: {}", e)))?;

        let has_metadata = next_plaid::filtering::exists(&path_str);

        Ok(crate::models::IndexSummary {
            name: name.to_string(),
            num_documents: metadata["num_documents"].as_u64().unwrap_or(0) as usize,
            num_embeddings: metadata["num_embeddings"].as_u64().unwrap_or(0) as usize,
            num_partitions: metadata["num_partitions"].as_u64().unwrap_or(0) as usize,
            dimension: metadata["embedding_dim"].as_u64().unwrap_or(0) as usize,
            nbits: metadata["nbits"].as_u64().unwrap_or(4) as usize,
            avg_doclen: metadata["avg_doclen"].as_f64().unwrap_or(0.0),
            has_metadata,
            max_documents,
        })
    }
}