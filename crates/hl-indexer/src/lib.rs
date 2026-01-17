//! hl-indexer: Business logic and data transformation for the trade ledger.
//!
//! This crate provides the [`Indexer`] struct which is the main entry point
//! for fetching, converting, and enriching trade data from Hyperliquid.
//!
//! # Overview
//!
//! The indexer:
//! - Fetches fills from Hyperliquid via `hl-ingestion`
//! - Converts raw API types to domain types (`hl-types`)
//! - Optionally enriches with builder attribution (with `builder-enrichment` feature)
//! - Calculates PnL for users
//!
//! # Example
//!
//! ```rust,no_run
//! use hl_indexer::{Indexer, IndexerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create indexer for mainnet
//!     let indexer = Indexer::mainnet();
//!
//!     // Fetch fills for a user
//!     let fills = indexer.get_user_fills(
//!         "0x1234567890abcdef1234567890abcdef12345678",
//!         Some(1704067200000), // from_ms
//!         None,               // to_ms (now)
//!     ).await?;
//!
//!     println!("Got {} fills", fills.len());
//!
//!     // Calculate PnL
//!     let pnl = indexer.get_user_pnl(
//!         "0x1234567890abcdef1234567890abcdef12345678",
//!         None,
//!         None,
//!         None, // all assets
//!     ).await?;
//!
//!     println!("Net PnL: {}", pnl.net_pnl);
//!     Ok(())
//! }
//! ```
//!
//! # Builder Enrichment
//!
//! Enable the `builder-enrichment` feature to add builder attribution:
//!
//! ```toml
//! [dependencies]
//! hl-indexer = { path = "../hl-indexer", features = ["builder-enrichment"] }
//! ```
//!
//! Then use `get_user_fills_with_builder_info()`:
//!
//! ```rust,ignore
//! let config = IndexerConfig::mainnet()
//!     .with_builder("0x2868fc0d9786a740b491577a43502259efa78a39");
//!
//! let indexer = Indexer::new(config);
//!
//! let result = indexer.get_user_fills_with_builder_info(
//!     "0x...",
//!     Some(from_ms),
//!     Some(to_ms),
//! ).await?;
//!
//! println!("Builder fills matched: {}", result.builder_fills_matched);
//! println!("Total builder fees: {}", result.total_builder_fees);
//! ```

mod converter;
mod error;
mod indexer;

pub use converter::{convert_fill, convert_fills};
pub use error::IndexerError;
pub use indexer::{FillSource, Indexer, IndexerConfig};

#[cfg(feature = "builder-enrichment")]
pub use indexer::EnrichedFillsResult;

// Re-export commonly used types from dependencies for convenience
pub use hl_ingestion::Network;
pub use hl_types::{Asset, PnLSummary, Position, Side, UserFill, UserPnL};
