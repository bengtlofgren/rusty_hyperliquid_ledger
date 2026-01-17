//! Hyperliquid data source implementation using hypersdk.
//!
//! This module provides `HyperliquidSource`, which wraps hypersdk's HTTP client
//! to implement our `DataSource` trait.
//!
//! # Design Decisions
//!
//! ## Thin Wrapper
//! We delegate directly to hypersdk methods without intermediate transformations.
//! This keeps overhead minimal - hypersdk already handles serialization, HTTP,
//! and error handling.
//!
//! ## No Stored State
//! The struct only holds the hypersdk client. No caching, no connection pools
//! beyond what hypersdk manages internally. Caching belongs in the indexer layer.
//!
//! ## Address Parsing
//! We parse the user address string on each call rather than storing it.
//! This keeps the API simple (strings are easier than `Address` types for callers)
//! and the parsing overhead is negligible compared to network latency.
//!
//! # Pagination
//!
//! When time parameters are provided, we use the direct API client with
//! `userFillsByTime` which supports pagination up to 10,000 fills.
//! Without time parameters, we fall back to hypersdk's `userFills` (max 500).

use crate::{api_client::ApiClient, config::Network, error::IngestionError, DataSource};
use hypersdk::hypercore::types::{ClearinghouseState, Fill, UserBalance};

/// Production data source for Hyperliquid using hypersdk.
///
/// # Example
///
/// ```rust,no_run
/// use hl_ingestion::{DataSource, HyperliquidSource};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let source = HyperliquidSource::mainnet();
///     let fills = source.get_user_fills("0x...", None, None).await?;
///     println!("Got {} fills", fills.len());
///     Ok(())
/// }
/// ```
pub struct HyperliquidSource {
    /// The underlying hypersdk HTTP client.
    /// hypersdk manages connection pooling and HTTP details internally.
    client: hypersdk::hypercore::http::Client,
    /// Direct API client for endpoints not exposed by hypersdk (e.g., userFillsByTime).
    api_client: ApiClient,
}

impl HyperliquidSource {
    /// Create a new source for the specified network.
    ///
    /// # Performance
    ///
    /// Client creation is cheap - no network connections are made until
    /// the first API call. The client uses HTTP/2 with connection pooling
    /// for efficient request handling.
    pub fn new(network: Network) -> Self {
        let (client, api_client) = match network {
            Network::Mainnet => (hypersdk::hypercore::mainnet(), ApiClient::mainnet()),
            Network::Testnet => (hypersdk::hypercore::testnet(), ApiClient::testnet()),
        };
        Self { client, api_client }
    }

    /// Create a source connected to Hyperliquid mainnet.
    ///
    /// Convenience method equivalent to `HyperliquidSource::new(Network::Mainnet)`.
    #[inline]
    pub fn mainnet() -> Self {
        Self::new(Network::Mainnet)
    }

    /// Create a source connected to Hyperliquid testnet.
    ///
    /// Convenience method equivalent to `HyperliquidSource::new(Network::Testnet)`.
    #[inline]
    pub fn testnet() -> Self {
        Self::new(Network::Testnet)
    }

    /// Parse a user address string into hypersdk's Address type.
    ///
    /// # Why a separate method?
    ///
    /// Address parsing is the same for all API calls, so we factor it out.
    /// This keeps the trait implementation methods focused on their specific logic.
    #[inline]
    fn parse_address(user: &str) -> Result<hypersdk::Address, IngestionError> {
        user.parse()
            .map_err(|_| IngestionError::InvalidAddress(user.to_string()))
    }
}

impl DataSource for HyperliquidSource {
    /// Fetch fills (trade history) for a user.
    ///
    /// # Time Window Filtering
    ///
    /// When `from_ms` is provided, we use the `userFillsByTime` API endpoint
    /// which supports pagination up to 10,000 fills. Without time parameters,
    /// we fall back to hypersdk's `userFills` (max 500 fills).
    ///
    /// # Returns
    ///
    /// Fills are returned in the order provided by the API (typically most
    /// recent first). The caller should sort if chronological order is needed.
    async fn get_user_fills(
        &self,
        user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
    ) -> Result<Vec<Fill>, IngestionError> {
        let address = Self::parse_address(user)?;

        // If time parameters are provided, use the paginated userFillsByTime API
        // which can return up to 10,000 fills.
        if let Some(start_time) = from_ms {
            let fills = self
                .api_client
                .user_fills_by_time(address, start_time, to_ms, false)
                .await?;
            return Ok(fills);
        }

        // No time params: fall back to hypersdk's userFills (simpler, max 500 fills).
        let all_fills = self.client.user_fills(address).await?;

        // Filter by to_ms if specified (from_ms is None here).
        let filtered: Vec<Fill> = if let Some(to) = to_ms {
            all_fills
                .into_iter()
                .filter(|fill| (fill.time as i64) <= to)
                .collect()
        } else {
            all_fills
        };

        Ok(filtered)
    }

    /// Fetch the user's clearinghouse state (perpetual positions and margin).
    ///
    /// This returns a snapshot of the user's current perpetual trading state,
    /// including:
    /// - Margin summary (account value, margin used, etc.)
    /// - All open positions with entry prices and sizes
    /// - Withdrawable balance
    ///
    /// # Note
    ///
    /// This returns the *current* state, not historical. For historical
    /// position reconstruction, use fills and compute positions.
    async fn get_clearinghouse_state(
        &self,
        user: &str,
    ) -> Result<ClearinghouseState, IngestionError> {
        let address = Self::parse_address(user)?;
        let state = self.client.clearinghouse_state(address).await?;
        Ok(state)
    }

    /// Fetch the user's spot token balances.
    ///
    /// Returns balances for all tokens the user holds, including both
    /// available and held (in open orders) amounts.
    async fn get_user_balances(&self, user: &str) -> Result<Vec<UserBalance>, IngestionError> {
        let address = Self::parse_address(user)?;
        let balances = self.client.user_balances(address).await?;
        Ok(balances)
    }
}
