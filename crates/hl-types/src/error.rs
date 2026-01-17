//! Error types for hl-types.

use thiserror::Error;

/// Errors that can occur when working with types.
#[derive(Debug, Error)]
pub enum TypeError {
    /// Invalid asset symbol.
    #[error("invalid asset: {0}")]
    InvalidAsset(String),

    /// Decimal parsing error.
    #[error("decimal error: {0}")]
    Decimal(#[from] rust_decimal::Error),
}
