//! Direct Hyperliquid API client for endpoints not exposed by hypersdk.
//!
//! This module provides a thin HTTP client that can make direct API calls
//! to Hyperliquid, bypassing hypersdk where needed. The primary use case
//! is accessing `userFillsByTime` with pagination support.
//!
//! # Design
//!
//! The client mimics hypersdk's patterns:
//! - Client struct with `reqwest::Client` + `base_url`
//! - `#[serde(tag = "type")]` for request enum
//! - POST to `/info` endpoint
//! - Error propagation via `?`
//!
//! # Pagination Strategy
//!
//! The Hyperliquid API returns max 2000 fills per request. To fetch more,
//! we loop backward in time:
//! 1. Request fills with `endTime` = requested end time
//! 2. If response has 2000 fills, set `endTime` = earliest fill time - 1
//! 3. Repeat until we reach `startTime` or get < 2000 fills
//! 4. Deduplicate by trade ID (`tid`) at page boundaries
//!
//! # Limitations
//!
//! - **10,000 fill maximum**: The API limits total retrievable fills
//! - **No builder attribution**: Fill data lacks builder field

use crate::error::IngestionError;
use hypersdk::hypercore::types::Fill;
use hypersdk::Address;
use serde::Serialize;
use std::collections::HashSet;
use url::Url;

/// Hyperliquid mainnet API base URL.
const MAINNET_URL: &str = "https://api.hyperliquid.xyz";

/// Hyperliquid testnet API base URL.
const TESTNET_URL: &str = "https://api.hyperliquid-testnet.xyz";

/// Maximum fills per API request (API limit).
const MAX_FILLS_PER_REQUEST: usize = 2000;

/// Maximum total fills we'll fetch (API limit for userFillsByTime).
const MAX_TOTAL_FILLS: usize = 10000;

/// Direct API client for Hyperliquid endpoints.
///
/// Use this client for endpoints that hypersdk doesn't expose or doesn't
/// support fully (e.g., pagination for userFillsByTime).
pub(crate) struct ApiClient {
    http_client: reqwest::Client,
    base_url: Url,
}

impl ApiClient {
    /// Create a client for Hyperliquid mainnet.
    pub fn mainnet() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            base_url: Url::parse(MAINNET_URL).expect("mainnet URL is valid"),
        }
    }

    /// Create a client for Hyperliquid testnet.
    pub fn testnet() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            base_url: Url::parse(TESTNET_URL).expect("testnet URL is valid"),
        }
    }

    /// Fetch fills with pagination support.
    ///
    /// This method uses the `userFillsByTime` endpoint which supports
    /// time-based pagination, allowing retrieval of more than the 500
    /// fills that the basic `userFills` endpoint returns.
    ///
    /// # Arguments
    ///
    /// * `user` - The user's address
    /// * `start_time` - Start of time window (inclusive), milliseconds since epoch
    /// * `end_time` - Optional end of time window (exclusive), defaults to now
    /// * `aggregate_by_time` - If true, aggregates fills at the same time
    ///
    /// # Pagination
    ///
    /// The method automatically handles pagination by looping backward in time
    /// until one of these conditions is met:
    /// - Empty response received
    /// - Less than 2000 fills returned (indicates no more data)
    /// - Earliest fill time <= start_time
    /// - Total fills >= 10,000 (API limit)
    ///
    /// # Returns
    ///
    /// Fills sorted by time descending (most recent first), deduplicated by `tid`.
    pub async fn user_fills_by_time(
        &self,
        user: Address,
        start_time: i64,
        end_time: Option<i64>,
        aggregate_by_time: bool,
    ) -> Result<Vec<Fill>, IngestionError> {
        let mut all_fills = Vec::new();
        let mut seen_tids: HashSet<u64> = HashSet::new();
        let mut current_end_time = end_time;

        // Convert start_time to u64 for comparison (API uses u64)
        let start_time_u64 = start_time.max(0) as u64;

        loop {
            // Build request
            let request = InfoRequest::UserFillsByTime {
                user: format!("{:?}", user), // Address Debug format gives 0x... string
                start_time: start_time_u64,
                end_time: current_end_time.map(|t| t.max(0) as u64),
                aggregate_by_time: if aggregate_by_time { Some(true) } else { None },
            };

            // Make API call
            let info_url = self.base_url.join("/info").expect("valid URL join");
            let response: Vec<Fill> = self
                .http_client
                .post(info_url)
                .json(&request)
                .send()
                .await?
                .error_for_status()
                .map_err(|e| IngestionError::Network(e.to_string()))?
                .json()
                .await?;

            // Empty response means no more data
            if response.is_empty() {
                break;
            }

            let response_len = response.len();

            // Deduplicate and collect fills
            for fill in response {
                if seen_tids.insert(fill.tid) {
                    all_fills.push(fill);
                }
            }

            // Check termination conditions
            // 1. Less than max fills returned -> no more data
            if response_len < MAX_FILLS_PER_REQUEST {
                break;
            }

            // 2. Reached total fill limit
            if all_fills.len() >= MAX_TOTAL_FILLS {
                tracing::warn!(
                    "Hit {} fill limit for user {:?}",
                    MAX_TOTAL_FILLS,
                    user
                );
                break;
            }

            // 3. Find earliest fill time for next iteration
            let earliest_time = all_fills
                .iter()
                .map(|f| f.time)
                .min()
                .unwrap_or(0);

            // 4. If earliest fill is at or before start_time, we're done
            if earliest_time <= start_time_u64 {
                break;
            }

            // Set up next iteration: end_time = earliest_time - 1
            current_end_time = Some(earliest_time as i64 - 1);
        }

        // Sort by time descending (most recent first) to match API behavior
        all_fills.sort_by(|a, b| b.time.cmp(&a.time));

        Ok(all_fills)
    }
}

/// Request types for the /info endpoint.
///
/// This enum mirrors hypersdk's approach of using a tagged enum for request types.
/// Each variant corresponds to a different API endpoint.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum InfoRequest {
    /// Fetch user fills within a time window.
    #[serde(rename_all = "camelCase")]
    UserFillsByTime {
        /// User address as hex string (e.g., "0x...")
        user: String,
        /// Start of time window (inclusive), milliseconds since epoch
        start_time: u64,
        /// Optional end of time window (exclusive), milliseconds since epoch
        #[serde(skip_serializing_if = "Option::is_none")]
        end_time: Option<u64>,
        /// If true, aggregates fills at the same timestamp
        #[serde(skip_serializing_if = "Option::is_none")]
        aggregate_by_time: Option<bool>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_request_serialization() {
        let request = InfoRequest::UserFillsByTime {
            user: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            start_time: 1704067200000,
            end_time: Some(1704153600000),
            aggregate_by_time: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"type\":\"userFillsByTime\""));
        assert!(json.contains("\"user\":\"0x1234567890abcdef1234567890abcdef12345678\""));
        assert!(json.contains("\"startTime\":1704067200000"));
        assert!(json.contains("\"endTime\":1704153600000"));
        // aggregate_by_time should be skipped when None
        assert!(!json.contains("aggregateByTime"));
    }

    #[test]
    fn test_info_request_without_end_time() {
        let request = InfoRequest::UserFillsByTime {
            user: "0xabc".to_string(),
            start_time: 1000,
            end_time: None,
            aggregate_by_time: Some(true),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("endTime"));
        assert!(json.contains("\"aggregateByTime\":true"));
    }
}
