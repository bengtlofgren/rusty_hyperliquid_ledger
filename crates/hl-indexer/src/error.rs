//! Error types for the indexer.

use thiserror::Error;

/// Errors that can occur during indexing operations.
#[derive(Debug, Error)]
pub enum IndexerError {
    /// Error from the ingestion layer.
    #[error("ingestion error: {0}")]
    Ingestion(#[from] hl_ingestion::IngestionError),

    /// Error from builder data (only with builder-enrichment feature).
    #[cfg(feature = "builder-enrichment")]
    #[error("builder data error: {0}")]
    BuilderData(#[from] hl_builder_data::BuilderDataError),

    /// Invalid address format.
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// Invalid time range.
    #[error("invalid time range: {0}")]
    InvalidTimeRange(String),

    /// No data available.
    #[error("no data available: {0}")]
    NoData(String),
}
