//! Error types for the ingestion layer.
//!
//! We use a simple enum with `thiserror` for ergonomic error handling.
//! No boxing of errors - each variant owns its data directly for
//! minimal overhead.

use thiserror::Error;

/// Errors that can occur during data ingestion.
///
/// This enum is intentionally simple - we convert external errors
/// (like `anyhow::Error` from hypersdk) into owned strings immediately.
/// This avoids:
/// - Generic type parameters on the error type
/// - Boxing/trait objects
/// - Lifetime complications
///
/// The trade-off is we lose the original error chain, but for our use case
/// (network errors from an external API) the string message is sufficient.
#[derive(Debug, Error)]
pub enum IngestionError {
    /// Network/HTTP errors from hypersdk.
    /// Contains the error message as an owned string.
    #[error("network error: {0}")]
    Network(String),

    /// Invalid user address format.
    /// The address string that failed to parse.
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// Configuration errors (e.g., missing env vars).
    #[error("config error: {0}")]
    Config(String),

    /// No data available (e.g., mock not configured).
    #[error("no data: {0}")]
    NoData(String),

    /// Invalid input (e.g., bad parameters).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// WebSocket connection error.
    #[error("websocket error: {0}")]
    WebSocket(String),
}

// Convert from anyhow::Error (what hypersdk returns) to our error type.
// We extract the message immediately to avoid storing the anyhow::Error
// which would require boxing.
impl From<anyhow::Error> for IngestionError {
    #[inline]
    fn from(err: anyhow::Error) -> Self {
        // Use Display formatting to get the full error chain as a string
        IngestionError::Network(format!("{:#}", err))
    }
}

// Convert from reqwest::Error (direct API calls) to our error type.
impl From<reqwest::Error> for IngestionError {
    #[inline]
    fn from(err: reqwest::Error) -> Self {
        IngestionError::Network(err.to_string())
    }
}
