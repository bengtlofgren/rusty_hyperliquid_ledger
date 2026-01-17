//! API error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// API errors that can be returned to clients.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Invalid request parameters.
    #[error("invalid request: {0}")]
    BadRequest(String),

    /// Resource not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Internal server error.
    #[error("internal error: {0}")]
    Internal(String),

    /// Error from the indexer layer.
    #[error("indexer error: {0}")]
    Indexer(#[from] hl_indexer::IndexerError),
}

/// Error response body.
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, details) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", Some(msg.clone())),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", Some(msg.clone())),
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", None)
            }
            ApiError::Indexer(e) => {
                tracing::error!("Indexer error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "indexer_error",
                    Some(e.to_string()),
                )
            }
        };

        let body = ErrorResponse {
            error: error.to_string(),
            details,
        };

        (status, Json(body)).into_response()
    }
}
