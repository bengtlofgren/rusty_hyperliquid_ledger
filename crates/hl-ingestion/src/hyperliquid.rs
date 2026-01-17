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
//! The underlying Hyperliquid API returns max 500 fills per request.
//! Currently, hypersdk's `user_fills` method does not expose pagination parameters.
//! This means we may only get the most recent 500 fills.
//!
//! TODO: Investigate if hypersdk supports pagination, or if we need to use
//! the raw API for historical data beyond 500 fills.

use crate::{config::Network, error::IngestionError, DataSource};
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
        let client = match network {
            Network::Mainnet => hypersdk::hypercore::mainnet(),
            Network::Testnet => hypersdk::hypercore::testnet(),
        };
        Self { client }
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
    /// The `from_ms` and `to_ms` parameters filter the results to a specific
    /// time window. Currently, this filtering happens client-side after
    /// receiving the data from hypersdk.
    ///
    /// # Pagination Limitation
    ///
    /// The underlying API returns max 500 fills. If a user has more fills
    /// in the requested time window, only the most recent 500 will be returned.
    /// This is a known limitation that may be addressed in a future version.
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

        // Fetch fills from hypersdk.
        // Note: This may only return the most recent 500 fills due to API limits.
        let all_fills = self.client.user_fills(address).await?;

        // Filter by time window if specified.
        // We do this client-side since hypersdk doesn't expose time params.
        let filtered: Vec<Fill> = all_fills
            .into_iter()
            .filter(|fill| {
                let t = fill.time as i64;
                let after_from = from_ms.map_or(true, |from| t >= from);
                let before_to = to_ms.map_or(true, |to| t <= to);
                after_from && before_to
            })
            .collect();

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
