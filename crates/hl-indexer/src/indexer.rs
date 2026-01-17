//! Core indexer implementation.
//!
//! The `Indexer` struct is the main entry point for fetching, converting,
//! and enriching trade data from Hyperliquid.

use crate::converter::convert_fills;
use crate::error::IndexerError;
use hl_ingestion::{CollectorHandle, DataSource, FillCollector, HyperliquidSource, Network};
use hl_types::{Asset, PnLSummary, UserFill, UserPnL};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "builder-enrichment")]
use hl_builder_data::{BuilderDataClient, FillEnricher};

#[cfg(feature = "builder-enrichment")]
use rust_decimal::Decimal;

/// Source for fetching fills.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FillSource {
    /// Use HTTP API (default). Limited to 10,000 historical fills.
    #[default]
    Api,
    /// Use WebSocket for real-time fill collection. No fill limit,
    /// but only captures fills from when the collector starts.
    WebSocket,
}

/// Configuration for the indexer.
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    /// Network to connect to (mainnet or testnet).
    pub network: Network,

    /// Source for fetching fills (API or WebSocket).
    pub fill_source: FillSource,

    /// Optional builder address for enrichment.
    /// Only used when builder-enrichment feature is enabled.
    pub builder_address: Option<String>,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            network: Network::Mainnet,
            fill_source: FillSource::default(),
            builder_address: None,
        }
    }
}

impl IndexerConfig {
    /// Create config for mainnet.
    pub fn mainnet() -> Self {
        Self {
            network: Network::Mainnet,
            fill_source: FillSource::default(),
            builder_address: None,
        }
    }

    /// Create config for testnet.
    pub fn testnet() -> Self {
        Self {
            network: Network::Testnet,
            fill_source: FillSource::default(),
            builder_address: None,
        }
    }

    /// Set the fill source (API or WebSocket).
    ///
    /// # Example
    ///
    /// ```rust
    /// use hl_indexer::{IndexerConfig, FillSource};
    ///
    /// // Use WebSocket for real-time fill collection
    /// let config = IndexerConfig::mainnet()
    ///     .with_fill_source(FillSource::WebSocket);
    /// ```
    pub fn with_fill_source(mut self, source: FillSource) -> Self {
        self.fill_source = source;
        self
    }

    /// Set the builder address for enrichment.
    #[cfg(feature = "builder-enrichment")]
    pub fn with_builder(mut self, address: &str) -> Self {
        self.builder_address = Some(address.to_string());
        self
    }
}

/// The main indexer for fetching and processing Hyperliquid trade data.
///
/// # Features
///
/// - Fetches fills from the Hyperliquid API via `hl-ingestion`
/// - Converts raw API types to domain types (`hl-types`)
/// - Optionally enriches with builder attribution (with `builder-enrichment` feature)
/// - Calculates PnL for users
/// - Supports WebSocket-based real-time fill collection (via `FillSource::WebSocket`)
///
/// # Example (HTTP API - default)
///
/// ```rust,no_run
/// use hl_indexer::{Indexer, IndexerConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let indexer = Indexer::new(IndexerConfig::mainnet());
///
///     // Fetch fills for a user (limited to 10k)
///     let fills = indexer.get_user_fills(
///         "0x1234...",
///         Some(1704067200000),
///         None
///     ).await?;
///
///     println!("Got {} fills", fills.len());
///     Ok(())
/// }
/// ```
///
/// # Example (WebSocket - real-time, no limit)
///
/// ```rust,no_run
/// use hl_indexer::{Indexer, IndexerConfig, FillSource};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Use WebSocket mode
///     let config = IndexerConfig::mainnet()
///         .with_fill_source(FillSource::WebSocket);
///     let indexer = Indexer::new(config);
///
///     // Start collecting fills BEFORE the competition
///     indexer.start_collecting("0x1234...").await?;
///
///     // ... competition runs ...
///
///     // Get all collected fills (no 10k limit!)
///     let fills = indexer.get_user_fills("0x1234...", None, None).await?;
///     println!("Collected {} fills", fills.len());
///
///     // Stop when done
///     indexer.stop_collecting().await;
///     Ok(())
/// }
/// ```
pub struct Indexer {
    /// The data source for fetching from Hyperliquid (API mode).
    source: Arc<HyperliquidSource>,

    /// WebSocket fill collector (WebSocket mode).
    fill_collector: FillCollector,

    /// Handle to the running collector task (if active).
    collector_handle: Arc<RwLock<Option<CollectorHandle>>>,

    /// Builder data client (only with builder-enrichment feature).
    #[cfg(feature = "builder-enrichment")]
    builder_client: Option<BuilderDataClient>,

    /// Configuration.
    config: IndexerConfig,
}

impl Indexer {
    /// Create a new indexer with the given configuration.
    pub fn new(config: IndexerConfig) -> Self {
        let source = Arc::new(HyperliquidSource::new(config.network));
        let fill_collector = FillCollector::new(config.network);

        #[cfg(feature = "builder-enrichment")]
        let builder_client = config
            .builder_address
            .as_ref()
            .and_then(|addr| BuilderDataClient::new(addr).ok());

        Self {
            source,
            fill_collector,
            collector_handle: Arc::new(RwLock::new(None)),
            #[cfg(feature = "builder-enrichment")]
            builder_client,
            config,
        }
    }

    /// Create a new indexer for mainnet.
    pub fn mainnet() -> Self {
        Self::new(IndexerConfig::mainnet())
    }

    /// Create a new indexer for testnet.
    pub fn testnet() -> Self {
        Self::new(IndexerConfig::testnet())
    }

    /// Get the underlying data source.
    pub fn source(&self) -> &HyperliquidSource {
        &self.source
    }

    /// Get the configuration.
    pub fn config(&self) -> &IndexerConfig {
        &self.config
    }

    /// Get the fill source mode.
    pub fn fill_source(&self) -> FillSource {
        self.config.fill_source
    }

    /// Check if using WebSocket mode.
    pub fn is_websocket_mode(&self) -> bool {
        self.config.fill_source == FillSource::WebSocket
    }

    /// Check if the WebSocket collector is currently running.
    pub async fn is_collecting(&self) -> bool {
        self.fill_collector.is_running().await
    }

    /// Start collecting fills via WebSocket.
    ///
    /// This must be called before fills will be captured in WebSocket mode.
    /// Call this method before the competition starts to capture all fills.
    ///
    /// # Errors
    ///
    /// Returns an error if the collector is already running or if the
    /// user address is invalid.
    pub async fn start_collecting(&self, user: &str) -> Result<(), IndexerError> {
        if self.config.fill_source != FillSource::WebSocket {
            return Err(IndexerError::InvalidTimeRange(
                "start_collecting requires FillSource::WebSocket mode".to_string(),
            ));
        }

        let handle = self.fill_collector.start(user).await?;

        let mut guard = self.collector_handle.write().await;
        *guard = Some(handle);

        tracing::info!("Started WebSocket fill collection for {}", user);
        Ok(())
    }

    /// Stop collecting fills via WebSocket.
    ///
    /// After stopping, you can still retrieve collected fills via `get_user_fills`.
    pub async fn stop_collecting(&self) {
        let mut guard = self.collector_handle.write().await;
        if let Some(handle) = guard.take() {
            handle.stop().await;
            tracing::info!("Stopped WebSocket fill collection");
        }
    }

    /// Get the number of fills collected via WebSocket.
    ///
    /// Returns 0 if not in WebSocket mode or if collection hasn't started.
    pub async fn collected_fill_count(&self) -> usize {
        self.fill_collector.fill_count().await
    }

    /// Clear all collected fills from the WebSocket collector.
    pub async fn clear_collected_fills(&self) {
        self.fill_collector.clear().await;
    }

    /// Check if builder enrichment is enabled and configured.
    #[cfg(feature = "builder-enrichment")]
    pub fn has_builder_enrichment(&self) -> bool {
        self.builder_client.is_some()
    }

    /// Check if builder enrichment is enabled and configured.
    #[cfg(not(feature = "builder-enrichment"))]
    pub fn has_builder_enrichment(&self) -> bool {
        false
    }

    /// Fetch fills for a user within a time range.
    ///
    /// Behavior depends on the configured `FillSource`:
    ///
    /// - **`FillSource::Api`** (default): Fetches from HTTP API. Limited to 10k fills.
    /// - **`FillSource::WebSocket`**: Returns fills from the WebSocket collector.
    ///   You must call `start_collecting()` first!
    ///
    /// # Arguments
    ///
    /// * `user` - User address (hex string with 0x prefix)
    /// * `from_ms` - Optional start time (milliseconds since epoch)
    /// * `to_ms` - Optional end time (milliseconds since epoch)
    ///
    /// # Returns
    ///
    /// A vector of `UserFill` structs.
    pub async fn get_user_fills(
        &self,
        user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
    ) -> Result<Vec<UserFill>, IndexerError> {
        match self.config.fill_source {
            FillSource::Api => {
                // Fetch raw fills from the API
                let raw_fills = self.source.get_user_fills(user, from_ms, to_ms).await?;

                // Convert to our domain types
                let fills = convert_fills(&raw_fills);

                tracing::debug!(
                    "Fetched {} fills for user {} via API ({} to {})",
                    fills.len(),
                    user,
                    from_ms.map(|t| t.to_string()).unwrap_or("start".to_string()),
                    to_ms.map(|t| t.to_string()).unwrap_or("now".to_string())
                );

                Ok(fills)
            }
            FillSource::WebSocket => {
                // Get fills from the WebSocket collector
                let raw_fills = match (from_ms, to_ms) {
                    (Some(from), Some(to)) => {
                        self.fill_collector
                            .get_fills_in_range(from as u64, to as u64)
                            .await
                    }
                    _ => self.fill_collector.get_fills().await,
                };

                // Convert to our domain types
                let fills = convert_fills(&raw_fills);

                tracing::debug!(
                    "Retrieved {} fills for user {} from WebSocket collector",
                    fills.len(),
                    user
                );

                Ok(fills)
            }
        }
    }

    /// Fetch fills from the HTTP API regardless of the configured fill source.
    ///
    /// Use this when you need to backfill historical data while in WebSocket mode.
    pub async fn get_user_fills_from_api(
        &self,
        user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
    ) -> Result<Vec<UserFill>, IndexerError> {
        let raw_fills = self.source.get_user_fills(user, from_ms, to_ms).await?;
        let fills = convert_fills(&raw_fills);

        tracing::debug!(
            "Fetched {} fills for user {} via API (explicit)",
            fills.len(),
            user
        );

        Ok(fills)
    }

    /// Fetch fills and calculate PnL for a user.
    ///
    /// # Arguments
    ///
    /// * `user` - User address (hex string)
    /// * `from_ms` - Optional start time (milliseconds since epoch)
    /// * `to_ms` - Optional end time (milliseconds since epoch)
    /// * `assets` - Optional list of assets to filter PnL calculation
    ///
    /// # Returns
    ///
    /// A `PnLSummary` with realized PnL, fees, and per-asset breakdown.
    pub async fn get_user_pnl(
        &self,
        user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
        assets: Option<&[Asset]>,
    ) -> Result<PnLSummary, IndexerError> {
        let fills = self.get_user_fills(user, from_ms, to_ms).await?;

        let mut pnl_tracker = UserPnL::new(user.to_string());
        pnl_tracker.add_fills(fills);

        let summary = pnl_tracker.calculate_pnl(assets);

        tracing::info!(
            "Calculated PnL for {}: realized={}, fees={}, net={}",
            user,
            summary.realized_pnl,
            summary.total_fees,
            summary.net_pnl
        );

        Ok(summary)
    }

    /// Fetch fills and build a PnL tracker for detailed analysis.
    ///
    /// Returns the `UserPnL` struct which can be used for more detailed
    /// queries like time-range filtering or per-asset analysis.
    pub async fn get_user_pnl_tracker(
        &self,
        user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
    ) -> Result<UserPnL, IndexerError> {
        let fills = self.get_user_fills(user, from_ms, to_ms).await?;

        let mut pnl_tracker = UserPnL::new(user.to_string());
        pnl_tracker.add_fills(fills);

        Ok(pnl_tracker)
    }

    /// Get fills with builder enrichment (only with builder-enrichment feature).
    ///
    /// This method fetches both regular fills and builder fills, then
    /// enriches the regular fills with builder attribution data.
    #[cfg(feature = "builder-enrichment")]
    pub async fn get_user_fills_with_builder_info(
        &self,
        user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
    ) -> Result<EnrichedFillsResult, IndexerError> {
        use chrono::{TimeZone, Utc};

        let fills = self.get_user_fills(user, from_ms, to_ms).await?;

        // If no builder client, return fills without enrichment
        let Some(builder_client) = &self.builder_client else {
            return Ok(EnrichedFillsResult {
                fills,
                builder_fills_matched: 0,
                total_builder_fees: Decimal::ZERO,
                enricher: None,
            });
        };

        // Determine date range for builder data
        let (start_date, end_date) = match (from_ms, to_ms) {
            (Some(from), Some(to)) => {
                let start = Utc
                    .timestamp_millis_opt(from)
                    .single()
                    .map(|dt| dt.date_naive())
                    .unwrap_or_else(|| Utc::now().date_naive());
                let end = Utc
                    .timestamp_millis_opt(to)
                    .single()
                    .map(|dt| dt.date_naive())
                    .unwrap_or_else(|| Utc::now().date_naive());
                (start, end)
            }
            (Some(from), None) => {
                let start = Utc
                    .timestamp_millis_opt(from)
                    .single()
                    .map(|dt| dt.date_naive())
                    .unwrap_or_else(|| Utc::now().date_naive());
                (start, Utc::now().date_naive())
            }
            _ => {
                // No time range specified, use fills' time range
                if fills.is_empty() {
                    return Ok(EnrichedFillsResult {
                        fills,
                        builder_fills_matched: 0,
                        total_builder_fees: Decimal::ZERO,
                        enricher: None,
                    });
                }
                let min_time = fills.iter().map(|f| f.timestamp_ms).min().unwrap() as i64;
                let max_time = fills.iter().map(|f| f.timestamp_ms).max().unwrap() as i64;
                let start = Utc
                    .timestamp_millis_opt(min_time)
                    .single()
                    .map(|dt| dt.date_naive())
                    .unwrap_or_else(|| Utc::now().date_naive());
                let end = Utc
                    .timestamp_millis_opt(max_time)
                    .single()
                    .map(|dt| dt.date_naive())
                    .unwrap_or_else(|| Utc::now().date_naive());
                (start, end)
            }
        };

        // Fetch builder fills
        let builder_fills = builder_client
            .fetch_fills_range(start_date, end_date)
            .await?;

        let enricher = FillEnricher::new(builder_fills);

        // Count matches
        let mut matched = 0;
        let mut total_fees = Decimal::ZERO;

        for fill in &fills {
            if let Some(builder_fill) = enricher.get_builder_fill(fill, user) {
                matched += 1;
                total_fees += builder_fill.builder_fee;
            }
        }

        tracing::info!(
            "Builder enrichment: {} of {} fills matched, total builder fees: {}",
            matched,
            fills.len(),
            total_fees
        );

        Ok(EnrichedFillsResult {
            fills,
            builder_fills_matched: matched,
            total_builder_fees: total_fees,
            enricher: Some(enricher),
        })
    }
}

/// Result of fetching fills with builder enrichment.
#[cfg(feature = "builder-enrichment")]
pub struct EnrichedFillsResult {
    /// The user's fills.
    pub fills: Vec<UserFill>,

    /// Number of fills that matched builder fills.
    pub builder_fills_matched: usize,

    /// Total builder fees from matched fills.
    pub total_builder_fees: Decimal,

    /// The enricher for detailed lookups (if builder data was fetched).
    pub enricher: Option<FillEnricher>,
}

#[cfg(feature = "builder-enrichment")]
impl EnrichedFillsResult {
    /// Check if a specific fill was from the builder.
    pub fn is_builder_fill(&self, fill: &UserFill, user: &str) -> bool {
        self.enricher
            .as_ref()
            .map(|e| e.is_builder_fill(fill, user))
            .unwrap_or(false)
    }

    /// Get builder fee for a specific fill.
    pub fn get_builder_fee(&self, fill: &UserFill, user: &str) -> Option<Decimal> {
        self.enricher.as_ref()?.get_builder_fee(fill, user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = IndexerConfig::default();
        assert!(matches!(config.network, Network::Mainnet));
        assert!(config.builder_address.is_none());
    }

    #[test]
    fn test_config_mainnet() {
        let config = IndexerConfig::mainnet();
        assert!(matches!(config.network, Network::Mainnet));
    }

    #[test]
    fn test_config_testnet() {
        let config = IndexerConfig::testnet();
        assert!(matches!(config.network, Network::Testnet));
    }

    #[cfg(feature = "builder-enrichment")]
    #[test]
    fn test_config_with_builder() {
        let config = IndexerConfig::mainnet().with_builder("0x123");
        assert_eq!(config.builder_address, Some("0x123".to_string()));
    }

    #[test]
    fn test_indexer_creation() {
        let indexer = Indexer::mainnet();
        assert!(matches!(indexer.config().network, Network::Mainnet));
    }

    #[test]
    fn test_fill_source_default() {
        let config = IndexerConfig::default();
        assert_eq!(config.fill_source, FillSource::Api);
    }

    #[test]
    fn test_fill_source_websocket() {
        let config = IndexerConfig::mainnet().with_fill_source(FillSource::WebSocket);
        assert_eq!(config.fill_source, FillSource::WebSocket);
    }

    #[test]
    fn test_indexer_fill_source() {
        let config = IndexerConfig::mainnet().with_fill_source(FillSource::WebSocket);
        let indexer = Indexer::new(config);
        assert!(indexer.is_websocket_mode());
        assert_eq!(indexer.fill_source(), FillSource::WebSocket);
    }

    #[test]
    fn test_indexer_api_mode() {
        let indexer = Indexer::mainnet();
        assert!(!indexer.is_websocket_mode());
        assert_eq!(indexer.fill_source(), FillSource::Api);
    }

    #[tokio::test]
    async fn test_collected_fill_count_starts_zero() {
        let config = IndexerConfig::mainnet().with_fill_source(FillSource::WebSocket);
        let indexer = Indexer::new(config);
        assert_eq!(indexer.collected_fill_count().await, 0);
    }
}
