//! # hl-ingestion
//!
//! Data ingestion layer for Hyperliquid APIs.
//!
//! This crate provides a [`DataSource`] trait abstraction over hypersdk,
//! enabling data fetching from Hyperliquid with a clean, testable interface.
//!
//! ## Design Principles
//!
//! - **Zero-cost async**: Uses native async traits (Rust 1.75+), avoiding
//!   the heap allocations that `async_trait` would require.
//!
//! - **Thin wrapper**: Delegates directly to hypersdk with minimal overhead.
//!   No intermediate types or transformations.
//!
//! - **Testable**: The [`MockSource`] implementation allows testing without
//!   network calls.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use hl_ingestion::{DataSource, HyperliquidSource, Network};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a source (mainnet by default)
//!     let source = HyperliquidSource::mainnet();
//!
//!     // Fetch fills for a user
//!     let fills = source.get_user_fills(
//!         "0xYourAddressHere",
//!         Some(1704067200000), // from_ms (optional)
//!         None,               // to_ms (optional)
//!     ).await?;
//!
//!     println!("Fetched {} fills", fills.len());
//!     Ok(())
//! }
//! ```
//!
//! ## Testing with MockSource
//!
//! ```rust
//! use hl_ingestion::{DataSource, MockSource};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mock = MockSource::new();
//!     // Configure with test data using builder methods
//!     // let mock = mock.with_fills(vec![...]);
//!
//!     let fills = mock.get_user_fills("0x...", None, None).await.unwrap();
//!     assert!(fills.is_empty()); // No fills configured
//! }
//! ```
//!
//! ## Pagination
//!
//! When `from_ms` is provided to [`get_user_fills`], we use the `userFillsByTime`
//! API which supports pagination up to 10,000 fills. Without time parameters,
//! we fall back to hypersdk's simpler `userFills` endpoint (max 500 fills).
//!
//! ## Real-Time Fill Collection (WebSocket)
//!
//! To bypass the 10,000 fill limit, use [`FillCollector`] to capture fills
//! in real-time via WebSocket. Start the collector before a competition
//! begins to capture all fills as they happen:
//!
//! ```rust,ignore
//! use hl_ingestion::{FillCollector, Network};
//!
//! // Create and start collector before competition
//! let collector = FillCollector::new(Network::Mainnet);
//! let handle = collector.start("0x...").await?;
//!
//! // ... competition runs ...
//!
//! // Get all collected fills (no 10k limit!)
//! let fills = collector.get_fills().await;
//! handle.stop().await;
//! ```
//!
//! ## Known Limitations
//!
//! ### Fill Limit (Historical API)
//!
//! The Hyperliquid API limits `userFillsByTime` to 10,000 fills maximum,
//! even with pagination. For users with more historical fills, only the
//! most recent 10,000 within the time window will be returned.
//!
//! **Workaround**: Use [`FillCollector`] to capture fills in real-time via
//! WebSocket, which has no fill limit.
//!
//! ### Builder Attribution
//!
//! The public API does not expose builder attribution in fill data.
//! The [`Fill`] struct lacks a `builder` field. Builder-only filtering
//! will require an alternative data source in the future.

mod api_client;
pub mod config;
pub mod error;
mod hyperliquid;
mod mock;
mod ws_collector;

// Re-export our types
pub use config::Network;
pub use error::IngestionError;
pub use hyperliquid::HyperliquidSource;
pub use mock::MockSource;
pub use ws_collector::{CollectorHandle, FillCollector};

// Re-export hypersdk types that appear in our public API.
// This allows downstream crates to use these types without adding
// hypersdk as a direct dependency.
pub use hypersdk::hypercore::types::{
    AssetPosition, ClearinghouseState, Fill, MarginSummary, PositionData, Side, UserBalance,
};

/// Data source abstraction for Hyperliquid data.
///
/// This trait defines the interface for fetching data from Hyperliquid.
/// It uses native async syntax (Rust 1.75+) rather than `async_trait`
/// to avoid heap allocations from `Box<dyn Future>`.
///
/// ## Time Parameters
///
/// Methods that accept `from_ms` and `to_ms` parameters filter results
/// to a specific time window:
///
/// - `from_ms`: Start of time window (inclusive), milliseconds since Unix epoch
/// - `to_ms`: End of time window (inclusive), milliseconds since Unix epoch
/// - If both are `None`, returns all available data (subject to API limits)
///
/// ## Implementors
///
/// - [`HyperliquidSource`]: Production implementation using hypersdk
/// - [`MockSource`]: Test implementation with configurable responses
///
/// ## Why `Send + Sync`?
///
/// The trait requires `Send + Sync` to allow data sources to be shared
/// across async tasks (e.g., stored in `Arc` and used from multiple handlers).
pub trait DataSource: Send + Sync {
    /// Fetch fills (trade executions) for a user within a time window.
    ///
    /// # Arguments
    ///
    /// * `user` - The user's address as a hex string (e.g., "0x...")
    /// * `from_ms` - Optional start of time window (inclusive)
    /// * `to_ms` - Optional end of time window (inclusive)
    ///
    /// # Returns
    ///
    /// A vector of [`Fill`] structs representing executed trades.
    /// Order is typically most-recent-first (as returned by the API).
    ///
    /// # Errors
    ///
    /// Returns [`IngestionError::InvalidAddress`] if the address cannot be parsed.
    /// Returns [`IngestionError::Network`] if the API call fails.
    fn get_user_fills(
        &self,
        user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
    ) -> impl std::future::Future<Output = Result<Vec<Fill>, IngestionError>> + Send;

    /// Fetch the user's clearinghouse state (perpetual positions and margin).
    ///
    /// Returns the current snapshot of the user's perpetual trading account,
    /// including positions, margin summary, and withdrawable balance.
    ///
    /// # Note
    ///
    /// This returns current state, not historical. For historical position
    /// reconstruction, fetch fills and compute positions from them.
    fn get_clearinghouse_state(
        &self,
        user: &str,
    ) -> impl std::future::Future<Output = Result<ClearinghouseState, IngestionError>> + Send;

    /// Fetch the user's spot token balances.
    ///
    /// Returns balances for all tokens the user holds, including both
    /// available (free) and held (in open orders) amounts.
    fn get_user_balances(
        &self,
        user: &str,
    ) -> impl std::future::Future<Output = Result<Vec<UserBalance>, IngestionError>> + Send;
}
