//! Request and response models for the next-plaid API.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// =============================================================================
// Index Management
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIndexRequest {
    #[schema(example = "my_index")]
    pub name: String,
    #[serde(default)]
    pub config: IndexConfigRequest,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct IndexConfigRequest {
    #[serde(default)]
    #[schema(example = 4)]
    pub nbits: Option<usize>,
    #[serde(default)]
    #[schema(example = 50000)]
    pub batch_size: Option<usize>,
    #[serde(default)]
    #[schema(example = 42)]
    pub seed: Option<u64>,
    #[serde(default)]
    #[schema(example = 999)]
    pub start_from_scratch: Option<usize>,
    #[serde(default)]
    #[schema(example = 10000)]
    pub max_documents: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateIndexResponse {
    #[schema(example = "my_index")]
    pub name: String,
    pub config: IndexConfigStored,
    #[schema(example = "Index declared. Use POST /indices/{name}/update to add documents.")]
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct IndexConfigStored {
    #[schema(example = 4)]
    pub nbits: usize,
    #[schema(example = 50000)]
    pub batch_size: usize,
    #[schema(example = 42)]
    pub seed: Option<u64>,
    #[serde(default = "default_start_from_scratch")]
    #[schema(example = 999)]
    pub start_from_scratch: usize,
    #[serde(default)]
    #[schema(example = 10000)]
    pub max_documents: Option<usize>,
}

fn default_start_from_scratch() -> usize {
    999
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexInfoResponse {
    #[schema(example = "my_index")]
    pub name: String,
    #[schema(example = 1000)]
    pub num_documents: usize,
    #[schema(example = 50000)]
    pub num_embeddings: usize,
    #[schema(example = 512)]
    pub num_partitions: usize,
    #[schema(example = 50.0)]
    pub avg_doclen: f64,
    #[schema(example = 128)]
    pub dimension: usize,
    #[schema(example = true)]
    pub has_metadata: bool,
    #[schema(example = 1000)]
    pub metadata_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 10000)]
    pub max_documents: Option<usize>,
}

// =============================================================================
// Document Upload
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct DocumentEmbeddings {
    #[schema(example = json!([[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]))]
    pub embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddDocumentsRequest {
    pub documents: Vec<DocumentEmbeddings>,
    pub metadata: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AddDocumentsResponse {
    #[schema(example = 10)]
    pub documents_added: usize,
    #[schema(example = 1010)]
    pub total_documents: usize,
    #[schema(example = 1000)]
    pub start_id: usize,
}

// =============================================================================
// Search
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct QueryEmbeddings {
    #[schema(example = json!([[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]))]
    pub embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchRequest {
    pub queries: Vec<QueryEmbeddings>,
    #[serde(default)]
    pub params: SearchParamsRequest,
    #[serde(default)]
    #[schema(example = json!([0, 5, 10, 15]))]
    pub subset: Option<Vec<i64>>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct SearchParamsRequest {
    #[serde(default)]
    #[schema(example = 10)]
    pub top_k: Option<usize>,
    #[serde(default)]
    #[schema(example = 8)]
    pub n_ivf_probe: Option<usize>,
    #[serde(default)]
    #[schema(example = 4096)]
    pub n_full_scores: Option<usize>,
    #[serde(default)]
    #[schema(example = 0.4)]
    pub centroid_score_threshold: Option<Option<f32>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QueryResultResponse {
    #[schema(example = 0)]
    pub query_id: usize,
    #[schema(example = json!([42, 17, 89, 5]))]
    pub document_ids: Vec<i64>,
    #[schema(example = json!([0.95, 0.87, 0.82, 0.75]))]
    pub scores: Vec<f32>,
    pub metadata: Vec<Option<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchResponse {
    pub results: Vec<QueryResultResponse>,
    #[schema(example = 1)]
    pub num_queries: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FilteredSearchRequest {
    pub queries: Vec<QueryEmbeddings>,
    #[serde(default)]
    pub params: SearchParamsRequest,
    #[schema(example = "category = ? AND score > ?")]
    pub filter_condition: String,
    #[serde(default)]
    #[schema(example = json!(["science", 90]))]
    pub filter_parameters: Vec<serde_json::Value>,
}

// =============================================================================
// Metadata
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct CheckMetadataRequest {
    #[schema(example = json!([0, 5, 10, 999]))]
    pub document_ids: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CheckMetadataResponse {
    #[schema(example = json!([0, 5, 10]))]
    pub existing_ids: Vec<i64>,
    #[schema(example = json!([999]))]
    pub missing_ids: Vec<i64>,
    #[schema(example = 3)]
    pub existing_count: usize,
    #[schema(example = 1)]
    pub missing_count: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GetMetadataRequest {
    #[serde(default)]
    #[schema(example = json!([0, 5, 10]))]
    pub document_ids: Option<Vec<i64>>,
    #[serde(default)]
    #[schema(example = "category = ?")]
    pub condition: Option<String>,
    #[serde(default)]
    #[schema(example = json!(["science"]))]
    pub parameters: Vec<serde_json::Value>,
    #[serde(default)]
    #[schema(example = 100)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GetMetadataResponse {
    pub metadata: Vec<serde_json::Value>,
    #[schema(example = 2)]
    pub count: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct QueryMetadataRequest {
    #[schema(example = "category = ? AND score > ?")]
    pub condition: String,
    #[serde(default)]
    #[schema(example = json!(["science", 90]))]
    pub parameters: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QueryMetadataResponse {
    #[schema(example = json!([0, 5, 42, 89]))]
    pub document_ids: Vec<i64>,
    #[schema(example = 4)]
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MetadataCountResponse {
    #[schema(example = 1000)]
    pub count: usize,
    #[schema(example = true)]
    pub has_metadata: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMetadataRequest {
    #[schema(example = "category = ? AND score > ?")]
    pub condition: String,
    #[serde(default)]
    #[schema(example = json!(["science", 90]))]
    pub parameters: Vec<serde_json::Value>,
    #[schema(example = json!({"status": "reviewed", "updated_at": "2024-01-15"}))]
    pub updates: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateMetadataResponse {
    #[schema(example = 5)]
    pub updated: usize,
}

// =============================================================================
// Delete
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteDocumentsRequest {
    #[schema(example = "category = ? AND year < ?")]
    pub condition: String,
    #[serde(default)]
    #[schema(example = json!(["outdated", 2020]))]
    pub parameters: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteDocumentsResponse {
    #[schema(example = 3)]
    pub deleted: usize,
    #[schema(example = 997)]
    pub remaining: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteIndexResponse {
    #[schema(example = true)]
    pub deleted: bool,
    #[schema(example = "my_index")]
    pub name: String,
}

// =============================================================================
// Update
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateIndexRequest {
    pub documents: Vec<DocumentEmbeddings>,
    pub metadata: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateIndexResponse {
    #[schema(example = "my_index")]
    pub name: String,
    #[schema(example = false)]
    pub created: bool,
    #[schema(example = 10)]
    pub documents_added: usize,
    #[schema(example = 1010)]
    pub total_documents: usize,
    #[schema(example = 50500)]
    pub num_embeddings: usize,
    #[schema(example = 512)]
    pub num_partitions: usize,
    #[schema(example = 128)]
    pub dimension: usize,
}

// =============================================================================
// Index Configuration
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateIndexConfigRequest {
    #[schema(example = 5000)]
    pub max_documents: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateIndexConfigResponse {
    #[schema(example = "my_index")]
    pub name: String,
    pub config: IndexConfigStored,
    #[schema(example = "max_documents set to 5000.")]
    pub message: String,
}

// =============================================================================
// Health
// =============================================================================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    #[schema(example = "healthy")]
    pub status: String,
    #[schema(example = "1.0.8")]
    pub version: String,
    #[schema(example = 2)]
    pub loaded_indices: usize,
    #[schema(example = "./indices")]
    pub index_dir: String,
    #[schema(example = 104857600)]
    pub memory_usage_bytes: u64,
    pub indices: Vec<IndexSummary>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexSummary {
    #[schema(example = "my_index")]
    pub name: String,
    #[schema(example = 1000)]
    pub num_documents: usize,
    #[schema(example = 50000)]
    pub num_embeddings: usize,
    #[schema(example = 512)]
    pub num_partitions: usize,
    #[schema(example = 128)]
    pub dimension: usize,
    #[schema(example = 4)]
    pub nbits: usize,
    #[schema(example = 50.0)]
    pub avg_doclen: f64,
    #[schema(example = true)]
    pub has_metadata: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 10000)]
    pub max_documents: Option<usize>,
}

// =============================================================================
// Error
// =============================================================================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    #[schema(example = "INDEX_NOT_FOUND")]
    pub code: String,
    #[schema(example = "Index 'my_index' not found")]
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}