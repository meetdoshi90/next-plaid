//! Error handling for the next_plaid API.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Index not found: {0}")]
    IndexNotFound(String),

    #[error("Index already exists: {0}")]
    IndexAlreadyExists(String),

    #[error("Index not declared: {0}. Call POST /indices first to declare the index.")]
    IndexNotDeclared(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Metadata database not found for index: {0}")]
    MetadataNotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Next-Plaid error: {0}")]
    NextPlaid(#[from] next_plaid::Error),
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            ApiError::IndexNotFound(msg) => (StatusCode::NOT_FOUND, "INDEX_NOT_FOUND", msg.clone()),
            ApiError::IndexAlreadyExists(msg) => {
                (StatusCode::CONFLICT, "INDEX_ALREADY_EXISTS", msg.clone())
            }
            ApiError::IndexNotDeclared(msg) => (
                StatusCode::NOT_FOUND,
                "INDEX_NOT_DECLARED",
                format!(
                    "Index '{}' not declared. Call POST /indices first to declare the index.",
                    msg
                ),
            ),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            ApiError::DimensionMismatch { expected, actual } => (
                StatusCode::BAD_REQUEST,
                "DIMENSION_MISMATCH",
                format!("Expected dimension {}, got {}", expected, actual),
            ),
            ApiError::MetadataNotFound(msg) => {
                (StatusCode::NOT_FOUND, "METADATA_NOT_FOUND", msg.clone())
            }
            ApiError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                msg.clone(),
            ),
            ApiError::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                msg.clone(),
            ),
            ApiError::NextPlaid(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "NEXT_PLAID_ERROR",
                e.to_string(),
            ),
        };

        let body = ErrorResponse {
            code,
            message,
            details: None,
        };

        (status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;