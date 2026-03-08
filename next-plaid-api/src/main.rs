//! Next-Plaid REST API Server
//!
//! A REST API for the next-plaid multi-vector search engine.
//!
//! # Endpoints
//!
//! ## Index Management
//! - `GET /indices` - List all indices
//! - `POST /indices` - Create a new index
//! - `GET /indices/{name}` - Get index info
//! - `DELETE /indices/{name}` - Delete an index
//! - `PUT /indices/{name}/config` - Update index config
//!
//! ## Documents
//! - `POST /indices/{name}/update` - Add documents with pre-computed embeddings
//! - `POST /indices/{name}/documents` - Add documents (legacy)
//! - `DELETE /indices/{name}/documents` - Delete documents
//!
//! ## Search
//! - `POST /indices/{name}/search` - Search with embeddings
//! - `POST /indices/{name}/search/filtered` - Search with metadata filter
//!
//! ## Metadata
//! - `GET /indices/{name}/metadata` - Get all metadata
//! - `GET /indices/{name}/metadata/count` - Get metadata count
//! - `POST /indices/{name}/metadata/check` - Check if documents have metadata
//! - `POST /indices/{name}/metadata/query` - Query metadata with SQL condition
//! - `POST /indices/{name}/metadata/get` - Get metadata for specific documents
//! - `POST /indices/{name}/metadata/update` - Update metadata with SQL condition
//!
//! ## Documentation
//! - `GET /swagger-ui` - Swagger UI
//! - `GET /api-docs/openapi.json` - OpenAPI specification

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower::limit::ConcurrencyLimitLayer;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::{
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod error;
mod handlers;
mod models;
mod state;
mod tracing_middleware;

use models::HealthResponse;
use next_plaid_api::PrettyJson;
use state::{ApiConfig, AppState};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Next-Plaid API",
        version = "1.0.8",
        description = "REST API for next-plaid multi-vector search engine.\n\nNext-Plaid implements the PLAID algorithm for efficient ColBERT-style retrieval with support for:\n- Multi-vector document embeddings\n- Batch query search\n- SQLite-based metadata filtering\n- Memory-mapped indices for low RAM usage",
        license(name = "Apache-2.0", url = "https://www.apache.org/licenses/LICENSE-2.0"),
    ),
    servers(
        (url = "/", description = "Local server")
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "indices", description = "Index management operations"),
        (name = "documents", description = "Document upload and deletion"),
        (name = "search", description = "Search operations"),
        (name = "metadata", description = "Metadata management and filtering"),
    ),
    paths(
        health,
        handlers::documents::list_indices,
        handlers::documents::create_index,
        handlers::documents::get_index_info,
        handlers::documents::delete_index,
        handlers::documents::add_documents,
        handlers::documents::delete_documents,
        handlers::documents::update_index,
        handlers::documents::update_index_config,
        handlers::search::search,
        handlers::search::search_filtered,
        handlers::metadata::get_all_metadata,
        handlers::metadata::get_metadata_count,
        handlers::metadata::check_metadata,
        handlers::metadata::query_metadata,
        handlers::metadata::get_metadata,
        handlers::metadata::update_metadata,
    ),
    components(schemas(
        models::HealthResponse,
        models::IndexSummary,
        models::ErrorResponse,
        models::CreateIndexRequest,
        models::CreateIndexResponse,
        models::IndexConfigRequest,
        models::IndexConfigStored,
        models::IndexInfoResponse,
        models::DocumentEmbeddings,
        models::AddDocumentsRequest,
        models::AddDocumentsResponse,
        models::DeleteDocumentsRequest,
        models::DeleteDocumentsResponse,
        models::DeleteIndexResponse,
        models::UpdateIndexRequest,
        models::UpdateIndexResponse,
        models::QueryEmbeddings,
        models::SearchRequest,
        models::SearchParamsRequest,
        models::SearchResponse,
        models::QueryResultResponse,
        models::FilteredSearchRequest,
        models::CheckMetadataRequest,
        models::CheckMetadataResponse,
        models::GetMetadataRequest,
        models::GetMetadataResponse,
        models::QueryMetadataRequest,
        models::QueryMetadataResponse,
        models::MetadataCountResponse,
        models::UpdateMetadataRequest,
        models::UpdateMetadataResponse,
        models::UpdateIndexConfigRequest,
        models::UpdateIndexConfigResponse,
    ))
)]
struct ApiDoc;

static SYSINFO_SYSTEM: std::sync::OnceLock<std::sync::Mutex<sysinfo::System>> =
    std::sync::OnceLock::new();

fn get_memory_usage_bytes() -> u64 {
    let pid = match sysinfo::get_current_pid() {
        Ok(pid) => pid,
        Err(_) => return 0,
    };
    let system_mutex = SYSINFO_SYSTEM.get_or_init(|| std::sync::Mutex::new(sysinfo::System::new()));
    let mut system = match system_mutex.lock() {
        Ok(guard) => guard,
        Err(_) => return 0,
    };
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).map(|p| p.memory()).unwrap_or(0)
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
async fn health(state: axum::extract::State<Arc<AppState>>) -> PrettyJson<HealthResponse> {
    if !state.config.index_dir.exists() {
        let dir = state.config.index_dir.clone();
        tokio::task::spawn_blocking(move || std::fs::create_dir_all(&dir).ok());
    }

    PrettyJson(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        loaded_indices: state.loaded_count(),
        index_dir: state.config.index_dir.to_string_lossy().to_string(),
        memory_usage_bytes: get_memory_usage_bytes(),
        indices: state.get_all_index_summaries(),
    })
}

fn rate_limit_error(_err: tower_governor::GovernorError) -> axum::http::Response<axum::body::Body> {
    let body = serde_json::json!({
        "code": "RATE_LIMITED",
        "message": "Too many requests. Please retry after the specified time.",
        "retry_after_seconds": 2
    });
    axum::http::Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("content-type", "application/json")
        .header("retry-after", "2")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!(signal = "SIGINT", "server.shutdown.initiated"),
        _ = terminate => tracing::info!(signal = "SIGTERM", "server.shutdown.initiated"),
    }
}

fn build_router(state: Arc<AppState>) -> Router {
    let rate_limit_enabled: bool = std::env::var("RATE_LIMIT_ENABLED")
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);
    let rate_limit_per_second: u64 = std::env::var("RATE_LIMIT_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    let rate_limit_burst_size: u32 = std::env::var("RATE_LIMIT_BURST_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let concurrency_limit: usize = std::env::var("CONCURRENCY_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    // Health — exempt from rate limiting
    let health_router = Router::new()
        .route("/health", get(health))
        .route("/", get(health))
        .layer(middleware::from_fn(tracing_middleware::trace_request))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .with_state(state.clone());

    // Index GET routes — exempt (clients poll these during async ops)
    let index_info_router = Router::new()
        .without_v07_checks()
        .route("/indices", get(handlers::list_indices))
        .route("/indices/{name}", get(handlers::get_index_info))
        .layer(middleware::from_fn(tracing_middleware::trace_request))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state.clone());

    // Update routes — exempt (per-index semaphore already provides backpressure)
    let update_router = Router::new()
        .without_v07_checks()
        .route("/indices/{name}/update", post(handlers::update_index))
        .layer(middleware::from_fn(tracing_middleware::trace_request))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(300),
        ))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(ConcurrencyLimitLayer::new(concurrency_limit))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024)) // 1GB
        .with_state(state.clone());

    // Delete routes — exempt (has internal batching)
    let delete_router = Router::new()
        .without_v07_checks()
        .route("/indices/{name}", delete(handlers::delete_index))
        .route("/indices/{name}/documents", delete(handlers::delete_documents))
        .layer(middleware::from_fn(tracing_middleware::trace_request))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(300),
        ))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(ConcurrencyLimitLayer::new(concurrency_limit))
        .with_state(state.clone());

    // Everything else — subject to rate limiting
    let api_router = Router::new()
        .without_v07_checks()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/indices", post(handlers::create_index))
        .route("/indices/{name}/documents", post(handlers::add_documents))
        .route("/indices/{name}/config", put(handlers::update_index_config))
        .route("/indices/{name}/search", post(handlers::search))
        .route("/indices/{name}/search/filtered", post(handlers::search_filtered))
        .route("/indices/{name}/metadata", get(handlers::get_all_metadata))
        .route("/indices/{name}/metadata/count", get(handlers::get_metadata_count))
        .route("/indices/{name}/metadata/check", post(handlers::check_metadata))
        .route("/indices/{name}/metadata/query", post(handlers::query_metadata))
        .route("/indices/{name}/metadata/get", post(handlers::get_metadata))
        .route("/indices/{name}/metadata/update", post(handlers::update_metadata))
        .layer(middleware::from_fn(tracing_middleware::trace_request))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(300),
        ))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let api_router = if rate_limit_enabled {
        let governor_conf = GovernorConfigBuilder::default()
            .per_second(rate_limit_per_second)
            .burst_size(rate_limit_burst_size)
            .finish()
            .expect("Failed to build rate limiter config");
        api_router.layer(GovernorLayer::new(governor_conf).error_handler(rate_limit_error))
    } else {
        api_router
    };

    let api_router = api_router
        .layer(ConcurrencyLimitLayer::new(concurrency_limit))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024)) // 1GB
        .with_state(state);

    Router::new()
        .merge(health_router)
        .merge(index_info_router)
        .merge(update_router)
        .merge(delete_router)
        .merge(api_router)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "next_plaid_api=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();

    let mut host = "0.0.0.0".to_string();
    let mut port: u16 = 8080;
    let mut index_dir = PathBuf::from("./indices");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--host" | "-h" => {
                host = args.get(i + 1).cloned().unwrap_or_else(|| {
                    eprintln!("Error: --host requires a value");
                    std::process::exit(1);
                });
                i += 2;
            }
            "--port" | "-p" => {
                port = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or_else(|| {
                    eprintln!("Error: --port requires a valid port number");
                    std::process::exit(1);
                });
                i += 2;
            }
            "--index-dir" | "-d" => {
                index_dir = PathBuf::from(args.get(i + 1).unwrap_or_else(|| {
                    eprintln!("Error: --index-dir requires a value");
                    std::process::exit(1);
                }));
                i += 2;
            }
            "--help" => {
                println!(
                    r#"Next-Plaid API Server

Usage: next-plaid-api [OPTIONS]

Options:
  -h, --host <HOST>      Host to bind to (default: 0.0.0.0)
  -p, --port <PORT>      Port to bind to (default: 8080)
  -d, --index-dir <DIR>  Directory for storing indices (default: ./indices)
  --help                 Show this help message

Environment Variables:
  RUST_LOG               Log level (e.g., RUST_LOG=debug)
  RATE_LIMIT_ENABLED     Enable rate limiting (true/1/yes)
  RATE_LIMIT_PER_SECOND  Requests per second when rate limiting enabled (default: 50)
  RATE_LIMIT_BURST_SIZE  Burst size when rate limiting enabled (default: 100)
  CONCURRENCY_LIMIT      Max concurrent in-flight requests (default: 100)

Examples:
  next-plaid-api
  next-plaid-api -p 3000 -d /data/indices
  RUST_LOG=debug next-plaid-api
"#
                );
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
        }
    }

    let config = ApiConfig {
        index_dir,
        default_top_k: 10,
    };

    tracing::info!(index_dir = %config.index_dir.display(), "server.starting");

    let state = Arc::new(AppState::new(config));
    let app = build_router(state);

    let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
    tracing::info!(
        listen_addr = %addr,
        swagger_ui = %format!("http://{}/swagger-ui", addr),
        "server.started"
    );

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();

    tracing::info!("server.shutdown.complete");
}