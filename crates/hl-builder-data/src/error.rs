//! Error types for builder data operations.

use thiserror::Error;

/// Errors that can occur when fetching or parsing builder data.
#[derive(Debug, Error)]
pub enum BuilderDataError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// No data available for the requested date (403/404).
    #[error("no data available for date {date}")]
    NotFound { date: String },

    /// LZ4 decompression failed.
    #[error("decompression error: {0}")]
    Decompression(String),

    /// CSV parsing failed.
    #[error("CSV parse error: {0}")]
    CsvParse(#[from] csv::Error),

    /// Invalid builder address format.
    #[error("invalid builder address: {0}")]
    InvalidAddress(String),

    /// Date parsing error.
    #[error("invalid date format: {0}")]
    InvalidDate(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
