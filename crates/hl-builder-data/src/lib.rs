//! hl-builder-data: Builder fill data fetcher and parser for Hyperliquid.
//!
//! This crate provides functionality to download, decompress, and parse
//! builder fill data from Hyperliquid's stats data endpoint.
//!
//! # Overview
//!
//! Builder fills are uploaded daily in LZ4-compressed CSV format to:
//! ```text
//! https://stats-data.hyperliquid.xyz/Mainnet/builder_fills/{builder_address}/{YYYYMMDD}.csv.lz4
//! ```
//!
//! **Important**: The builder address must be entirely lowercase.
//!
//! # Example
//!
//! ```rust,no_run
//! use hl_builder_data::{BuilderDataClient, FillEnricher};
//! use chrono::NaiveDate;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client for a specific builder
//!     let client = BuilderDataClient::new(
//!         "0x2868fc0d9786a740b491577a43502259efa78a39"
//!     )?;
//!
//!     // Fetch fills for a specific date
//!     let fills = client.fetch_fills(
//!         NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()
//!     ).await?;
//!
//!     println!("Found {} builder fills", fills.len());
//!
//!     // Create enricher for matching with regular fills
//!     let enricher = FillEnricher::new(fills);
//!     println!("Total builder fees: {}", enricher.total_builder_fees());
//!
//!     Ok(())
//! }
//! ```
//!
//! # Data Enrichment
//!
//! The [`FillEnricher`] can be used to match regular fills (from `hl-ingestion`)
//! with builder fills to determine if a trade was routed through a specific builder.
//!
//! ```rust,ignore
//! use hl_builder_data::FillEnricher;
//!
//! let enricher = FillEnricher::new(builder_fills);
//!
//! // Check if a fill was from the tracked builder
//! if enricher.is_builder_fill(&user_fill, "0x...") {
//!     let fee = enricher.get_builder_fee(&user_fill, "0x...");
//!     println!("Builder fee: {:?}", fee);
//! }
//! ```
//!
//! # Limitations
//!
//! - Files are uploaded with ~24 hour delay
//! - Returns 403 if no fills exist for that builder on that date
//! - No trade ID in CSV, so matching uses composite key (user, coin, time, size, price, side)

mod client;
mod enricher;
mod error;
mod parser;
mod types;

pub use client::BuilderDataClient;
pub use enricher::FillEnricher;
pub use error::BuilderDataError;
pub use types::{BuilderFill, BuilderFillSide};

// Re-export chrono::NaiveDate for convenience
pub use chrono::NaiveDate;
