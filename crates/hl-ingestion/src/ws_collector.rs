//! WebSocket-based fill collector for real-time fill capture.
//!
//! This module provides a [`FillCollector`] that connects to Hyperliquid's WebSocket
//! API and captures fills in real-time. This bypasses the 10,000 fill limit of the
//! historical API by capturing fills as they happen.
//!
//! # Usage
//!
//! Start the collector before the competition begins to capture all fills:
//!
//! ```rust,ignore
//! use hl_ingestion::{FillCollector, Network};
//!
//! let collector = FillCollector::new(Network::Mainnet);
//!
//! // Start collecting fills for a user
//! let handle = collector.start("0x...").await?;
//!
//! // ... competition runs ...
//!
//! // Get all collected fills
//! let fills = collector.get_fills();
//! println!("Collected {} fills", fills.len());
//!
//! // Stop collecting
//! handle.stop().await;
//! ```

use crate::error::IngestionError;
use crate::Network;
use hypersdk::hypercore::types::{Fill, Incoming, Subscription};
use hypersdk::hypercore::ws::Connection;
use hypersdk::Address;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

/// WebSocket URLs for Hyperliquid.
const MAINNET_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const TESTNET_WS_URL: &str = "wss://api.hyperliquid-testnet.xyz/ws";

/// A collector that captures fills in real-time via WebSocket.
///
/// The collector maintains a thread-safe store of all fills received,
/// deduplicated by trade ID. It automatically handles reconnection and
/// re-subscription.
#[derive(Clone)]
pub struct FillCollector {
    /// Network to connect to.
    network: Network,
    /// Thread-safe storage for fills, keyed by trade ID.
    fills: Arc<RwLock<HashMap<u64, Fill>>>,
    /// Whether the collector is currently running.
    is_running: Arc<RwLock<bool>>,
}

impl FillCollector {
    /// Create a new fill collector for the specified network.
    pub fn new(network: Network) -> Self {
        Self {
            network,
            fills: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a collector for mainnet.
    pub fn mainnet() -> Self {
        Self::new(Network::Mainnet)
    }

    /// Create a collector for testnet.
    pub fn testnet() -> Self {
        Self::new(Network::Testnet)
    }

    /// Get the WebSocket URL for the configured network.
    fn ws_url(&self) -> Url {
        match self.network {
            Network::Mainnet => Url::parse(MAINNET_WS_URL).expect("valid mainnet ws url"),
            Network::Testnet => Url::parse(TESTNET_WS_URL).expect("valid testnet ws url"),
        }
    }

    /// Start collecting fills for the specified user.
    ///
    /// This spawns a background task that connects to the WebSocket and
    /// stores all incoming fills. The task automatically handles reconnection.
    ///
    /// Returns a [`CollectorHandle`] that can be used to stop the collector.
    pub async fn start(&self, user: &str) -> Result<CollectorHandle, IngestionError> {
        // Check if already running
        {
            let is_running = self.is_running.read().await;
            if *is_running {
                return Err(IngestionError::WebSocket(
                    "Collector is already running".to_string(),
                ));
            }
        }

        // Parse user address
        let user_address: Address = user
            .parse()
            .map_err(|e| IngestionError::InvalidInput(format!("Invalid address: {}", e)))?;

        // Mark as running
        {
            let mut is_running = self.is_running.write().await;
            *is_running = true;
        }

        // Create connection
        let url = self.ws_url();
        let connection = Connection::new(url);

        // Subscribe to user fills
        connection.subscribe(Subscription::UserFills { user: user_address });

        tracing::info!("Started fill collector for user {} on {:?}", user, self.network);

        // Spawn background task
        let fills_store = self.fills.clone();
        let is_running = self.is_running.clone();
        let user_str = user.to_string();

        let task_handle = tokio::spawn(async move {
            use futures::StreamExt;

            let mut conn = connection;
            let mut total_received = 0usize;

            loop {
                // Check if we should stop
                {
                    let running = is_running.read().await;
                    if !*running {
                        tracing::info!("Fill collector stopping (received {} fills total)", total_received);
                        break;
                    }
                }

                // Wait for next message
                match conn.next().await {
                    Some(Incoming::UserFills { user: _, fills }) => {
                        let fill_count = fills.len();
                        if fill_count > 0 {
                            let mut store = fills_store.write().await;
                            for fill in fills {
                                store.insert(fill.tid, fill);
                            }
                            total_received += fill_count;
                            tracing::debug!(
                                "Received {} fills for {}, total stored: {}",
                                fill_count,
                                user_str,
                                store.len()
                            );
                        }
                    }
                    Some(Incoming::SubscriptionResponse(_)) => {
                        tracing::debug!("Subscription confirmed for {}", user_str);
                    }
                    Some(Incoming::Ping) | Some(Incoming::Pong) => {
                        // Heartbeat messages, ignore
                    }
                    Some(other) => {
                        tracing::trace!("Received other message type: {:?}", other);
                    }
                    None => {
                        // Connection closed, hypersdk should auto-reconnect
                        tracing::warn!("WebSocket connection closed, waiting for reconnect...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }

            // Close connection when stopping
            conn.close();
        });

        Ok(CollectorHandle {
            is_running: self.is_running.clone(),
            task_handle,
        })
    }

    /// Get all collected fills.
    ///
    /// Returns a vector of fills sorted by timestamp.
    pub async fn get_fills(&self) -> Vec<Fill> {
        let store = self.fills.read().await;
        let mut fills: Vec<Fill> = store.values().cloned().collect();
        fills.sort_by_key(|f| f.time);
        fills
    }

    /// Get the number of collected fills.
    pub async fn fill_count(&self) -> usize {
        self.fills.read().await.len()
    }

    /// Clear all collected fills.
    pub async fn clear(&self) {
        self.fills.write().await.clear();
    }

    /// Check if the collector is currently running.
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get fills within a time range.
    pub async fn get_fills_in_range(&self, from_ms: u64, to_ms: u64) -> Vec<Fill> {
        let store = self.fills.read().await;
        let mut fills: Vec<Fill> = store
            .values()
            .filter(|f| f.time >= from_ms && f.time <= to_ms)
            .cloned()
            .collect();
        fills.sort_by_key(|f| f.time);
        fills
    }

    /// Get fills for a specific asset.
    pub async fn get_fills_for_asset(&self, asset: &str) -> Vec<Fill> {
        let store = self.fills.read().await;
        let mut fills: Vec<Fill> = store
            .values()
            .filter(|f| f.coin.eq_ignore_ascii_case(asset))
            .cloned()
            .collect();
        fills.sort_by_key(|f| f.time);
        fills
    }
}

/// Handle for controlling a running fill collector.
pub struct CollectorHandle {
    is_running: Arc<RwLock<bool>>,
    task_handle: tokio::task::JoinHandle<()>,
}

impl CollectorHandle {
    /// Stop the collector gracefully.
    pub async fn stop(self) {
        // Signal to stop
        {
            let mut is_running = self.is_running.write().await;
            *is_running = false;
        }

        // Wait for task to complete
        let _ = self.task_handle.await;
        tracing::info!("Fill collector stopped");
    }

    /// Check if the collector is still running.
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector_creation() {
        let collector = FillCollector::mainnet();
        assert!(matches!(collector.network, Network::Mainnet));
    }

    #[test]
    fn test_ws_url() {
        let mainnet = FillCollector::mainnet();
        assert_eq!(mainnet.ws_url().as_str(), MAINNET_WS_URL);

        let testnet = FillCollector::testnet();
        assert_eq!(testnet.ws_url().as_str(), TESTNET_WS_URL);
    }

    #[tokio::test]
    async fn test_fill_count_empty() {
        let collector = FillCollector::mainnet();
        assert_eq!(collector.fill_count().await, 0);
    }

    #[tokio::test]
    async fn test_clear() {
        let collector = FillCollector::mainnet();
        // Can clear even when empty
        collector.clear().await;
        assert_eq!(collector.fill_count().await, 0);
    }
}
