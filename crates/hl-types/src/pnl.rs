//! PnL (Profit and Loss) tracking types.
//!
//! This module provides [`UserPnL`], a comprehensive PnL tracker that stores
//! all fills partitioned by asset and provides methods to calculate PnL.
//!
//! # Example
//!
//! ```rust
//! use hl_types::{Asset, UserPnL, UserFill};
//! use rust_decimal_macros::dec;
//!
//! let mut pnl = UserPnL::new("0x1234...".to_string());
//!
//! // Add fills as they come in
//! // pnl.add_fill(fill);
//!
//! // Calculate total PnL
//! let summary = pnl.calculate_pnl(None);
//!
//! // Calculate PnL for specific assets
//! let btc_only = pnl.calculate_pnl(Some(&[Asset::Btc]));
//! ```

use crate::{Asset, UserFill};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Summary of PnL calculation results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PnLSummary {
    /// Total realized PnL across all fills.
    pub realized_pnl: Decimal,

    /// Total fees paid.
    pub total_fees: Decimal,

    /// Net PnL (realized - fees).
    pub net_pnl: Decimal,

    /// Total number of fills.
    pub fill_count: usize,

    /// Total trading volume (sum of notional values).
    pub total_volume: Decimal,

    /// PnL breakdown by asset.
    pub by_asset: HashMap<Asset, AssetPnL>,
}

/// PnL summary for a single asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetPnL {
    /// The asset.
    pub asset: Asset,

    /// Realized PnL for this asset.
    pub realized_pnl: Decimal,

    /// Fees paid for this asset.
    pub fees: Decimal,

    /// Net PnL for this asset.
    pub net_pnl: Decimal,

    /// Number of fills for this asset.
    pub fill_count: usize,

    /// Trading volume for this asset.
    pub volume: Decimal,

    /// First fill timestamp (if any).
    pub first_fill_ms: Option<u64>,

    /// Last fill timestamp (if any).
    pub last_fill_ms: Option<u64>,
}

impl AssetPnL {
    /// Create a new empty AssetPnL for an asset.
    pub fn new(asset: Asset) -> Self {
        Self {
            asset,
            realized_pnl: Decimal::ZERO,
            fees: Decimal::ZERO,
            net_pnl: Decimal::ZERO,
            fill_count: 0,
            volume: Decimal::ZERO,
            first_fill_ms: None,
            last_fill_ms: None,
        }
    }
}

/// Comprehensive PnL tracker for a user.
///
/// Stores all fills partitioned by asset and provides methods to calculate
/// realized PnL, fees, and other trading metrics.
///
/// # Design
///
/// Fills are stored in a `HashMap<Asset, Vec<UserFill>>` for efficient
/// per-asset lookups. The `calculate_pnl` method can filter by specific
/// assets or calculate across all assets.
///
/// # Thread Safety
///
/// This struct is not thread-safe. For concurrent access, wrap in a mutex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPnL {
    /// The user's address (hex string).
    pub user: String,

    /// Fills partitioned by asset.
    fills_by_asset: HashMap<Asset, Vec<UserFill>>,

    /// Total fill count (cached for efficiency).
    total_fill_count: usize,
}

impl UserPnL {
    /// Create a new empty PnL tracker for a user.
    pub fn new(user: String) -> Self {
        Self {
            user,
            fills_by_asset: HashMap::new(),
            total_fill_count: 0,
        }
    }

    /// Add a fill to the tracker.
    pub fn add_fill(&mut self, fill: UserFill) {
        let asset = fill.asset.clone();
        self.fills_by_asset
            .entry(asset)
            .or_default()
            .push(fill);
        self.total_fill_count += 1;
    }

    /// Add multiple fills to the tracker.
    pub fn add_fills(&mut self, fills: impl IntoIterator<Item = UserFill>) {
        for fill in fills {
            self.add_fill(fill);
        }
    }

    /// Get all fills for a specific asset.
    pub fn fills_for_asset(&self, asset: &Asset) -> Option<&[UserFill]> {
        self.fills_by_asset.get(asset).map(|v| v.as_slice())
    }

    /// Get all fills, ordered by timestamp.
    pub fn all_fills(&self) -> Vec<&UserFill> {
        let mut fills: Vec<_> = self
            .fills_by_asset
            .values()
            .flatten()
            .collect();
        fills.sort_by_key(|f| f.timestamp_ms);
        fills
    }

    /// Get all assets that have fills.
    pub fn assets(&self) -> Vec<&Asset> {
        self.fills_by_asset.keys().collect()
    }

    /// Get the total number of fills.
    pub fn fill_count(&self) -> usize {
        self.total_fill_count
    }

    /// Check if there are any fills.
    pub fn is_empty(&self) -> bool {
        self.total_fill_count == 0
    }

    /// Calculate PnL for specified assets, or all assets if None.
    ///
    /// # Arguments
    ///
    /// * `assets` - Optional slice of assets to calculate PnL for.
    ///              If None, calculates for all assets.
    ///
    /// # Returns
    ///
    /// A [`PnLSummary`] containing total and per-asset PnL breakdown.
    ///
    /// # Example
    ///
    /// ```rust
    /// use hl_types::{Asset, UserPnL};
    ///
    /// let pnl = UserPnL::new("0x123".to_string());
    ///
    /// // Calculate total PnL
    /// let total = pnl.calculate_pnl(None);
    ///
    /// // Calculate PnL for BTC and ETH only
    /// let btc_eth = pnl.calculate_pnl(Some(&[Asset::Btc, Asset::Eth]));
    /// ```
    pub fn calculate_pnl(&self, assets: Option<&[Asset]>) -> PnLSummary {
        let mut summary = PnLSummary {
            realized_pnl: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            net_pnl: Decimal::ZERO,
            fill_count: 0,
            total_volume: Decimal::ZERO,
            by_asset: HashMap::new(),
        };

        // Determine which assets to process
        let assets_to_process: Vec<&Asset> = match assets {
            Some(filter) => filter.iter().collect(),
            None => self.fills_by_asset.keys().collect(),
        };

        for asset in assets_to_process {
            if let Some(fills) = self.fills_by_asset.get(asset) {
                let asset_pnl = self.calculate_asset_pnl(asset, fills);

                // Update totals
                summary.realized_pnl += asset_pnl.realized_pnl;
                summary.total_fees += asset_pnl.fees;
                summary.fill_count += asset_pnl.fill_count;
                summary.total_volume += asset_pnl.volume;

                // Store per-asset breakdown
                summary.by_asset.insert(asset.clone(), asset_pnl);
            }
        }

        // Calculate net PnL
        summary.net_pnl = summary.realized_pnl - summary.total_fees;

        summary
    }

    /// Calculate PnL for a single asset's fills.
    fn calculate_asset_pnl(&self, asset: &Asset, fills: &[UserFill]) -> AssetPnL {
        let mut pnl = AssetPnL::new(asset.clone());

        for fill in fills {
            pnl.realized_pnl += fill.closed_pnl;
            pnl.fees += fill.fee;
            pnl.fill_count += 1;
            pnl.volume += fill.notional_value();

            // Track time range
            match pnl.first_fill_ms {
                None => pnl.first_fill_ms = Some(fill.timestamp_ms),
                Some(first) if fill.timestamp_ms < first => {
                    pnl.first_fill_ms = Some(fill.timestamp_ms)
                }
                _ => {}
            }
            match pnl.last_fill_ms {
                None => pnl.last_fill_ms = Some(fill.timestamp_ms),
                Some(last) if fill.timestamp_ms > last => {
                    pnl.last_fill_ms = Some(fill.timestamp_ms)
                }
                _ => {}
            }
        }

        pnl.net_pnl = pnl.realized_pnl - pnl.fees;
        pnl
    }

    /// Calculate PnL within a time range.
    ///
    /// # Arguments
    ///
    /// * `from_ms` - Start of time range (inclusive), milliseconds since epoch
    /// * `to_ms` - End of time range (inclusive), milliseconds since epoch
    /// * `assets` - Optional slice of assets to filter by
    pub fn calculate_pnl_in_range(
        &self,
        from_ms: u64,
        to_ms: u64,
        assets: Option<&[Asset]>,
    ) -> PnLSummary {
        // Filter fills by time range first
        let mut filtered = UserPnL::new(self.user.clone());

        let assets_to_check: Vec<&Asset> = match assets {
            Some(filter) => filter.iter().collect(),
            None => self.fills_by_asset.keys().collect(),
        };

        for asset in assets_to_check {
            if let Some(fills) = self.fills_by_asset.get(asset) {
                for fill in fills {
                    if fill.timestamp_ms >= from_ms && fill.timestamp_ms <= to_ms {
                        filtered.add_fill(fill.clone());
                    }
                }
            }
        }

        filtered.calculate_pnl(None)
    }

    /// Get time range of all fills.
    /// Returns (first_fill_ms, last_fill_ms) or None if no fills.
    pub fn time_range(&self) -> Option<(u64, u64)> {
        let fills = self.all_fills();
        if fills.is_empty() {
            return None;
        }

        let first = fills.iter().map(|f| f.timestamp_ms).min()?;
        let last = fills.iter().map(|f| f.timestamp_ms).max()?;
        Some((first, last))
    }

    /// Clear all fills.
    pub fn clear(&mut self) {
        self.fills_by_asset.clear();
        self.total_fill_count = 0;
    }
}

impl Default for UserPnL {
    fn default() -> Self {
        Self::new(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill::Side;
    use rust_decimal_macros::dec;

    fn make_fill(asset: Asset, closed_pnl: Decimal, fee: Decimal, timestamp_ms: u64) -> UserFill {
        UserFill {
            asset,
            timestamp_ms,
            price: dec!(100),
            size: dec!(1),
            side: Side::Buy,
            fee,
            closed_pnl,
            trade_id: timestamp_ms,
            order_id: timestamp_ms,
            crossed: true,
            direction: "Open Long".to_string(),
        }
    }

    #[test]
    fn test_empty_pnl() {
        let pnl = UserPnL::new("0x123".to_string());
        assert!(pnl.is_empty());
        assert_eq!(pnl.fill_count(), 0);

        let summary = pnl.calculate_pnl(None);
        assert_eq!(summary.realized_pnl, Decimal::ZERO);
        assert_eq!(summary.total_fees, Decimal::ZERO);
        assert_eq!(summary.net_pnl, Decimal::ZERO);
    }

    #[test]
    fn test_add_fill() {
        let mut pnl = UserPnL::new("0x123".to_string());
        let fill = make_fill(Asset::Btc, dec!(100), dec!(1), 1000);

        pnl.add_fill(fill);

        assert_eq!(pnl.fill_count(), 1);
        assert!(!pnl.is_empty());
        assert_eq!(pnl.fills_for_asset(&Asset::Btc).unwrap().len(), 1);
    }

    #[test]
    fn test_calculate_pnl_all_assets() {
        let mut pnl = UserPnL::new("0x123".to_string());

        pnl.add_fill(make_fill(Asset::Btc, dec!(100), dec!(1), 1000));
        pnl.add_fill(make_fill(Asset::Btc, dec!(50), dec!(1), 2000));
        pnl.add_fill(make_fill(Asset::Eth, dec!(-25), dec!(0.5), 1500));

        let summary = pnl.calculate_pnl(None);

        assert_eq!(summary.realized_pnl, dec!(125)); // 100 + 50 - 25
        assert_eq!(summary.total_fees, dec!(2.5)); // 1 + 1 + 0.5
        assert_eq!(summary.net_pnl, dec!(122.5)); // 125 - 2.5
        assert_eq!(summary.fill_count, 3);
        assert_eq!(summary.by_asset.len(), 2);
    }

    #[test]
    fn test_calculate_pnl_filtered() {
        let mut pnl = UserPnL::new("0x123".to_string());

        pnl.add_fill(make_fill(Asset::Btc, dec!(100), dec!(1), 1000));
        pnl.add_fill(make_fill(Asset::Eth, dec!(50), dec!(1), 2000));
        pnl.add_fill(make_fill(Asset::Sol, dec!(25), dec!(0.5), 1500));

        // Only BTC
        let btc_only = pnl.calculate_pnl(Some(&[Asset::Btc]));
        assert_eq!(btc_only.realized_pnl, dec!(100));
        assert_eq!(btc_only.fill_count, 1);

        // BTC and ETH
        let btc_eth = pnl.calculate_pnl(Some(&[Asset::Btc, Asset::Eth]));
        assert_eq!(btc_eth.realized_pnl, dec!(150));
        assert_eq!(btc_eth.fill_count, 2);
    }

    #[test]
    fn test_calculate_pnl_in_range() {
        let mut pnl = UserPnL::new("0x123".to_string());

        pnl.add_fill(make_fill(Asset::Btc, dec!(100), dec!(1), 1000));
        pnl.add_fill(make_fill(Asset::Btc, dec!(50), dec!(1), 2000));
        pnl.add_fill(make_fill(Asset::Btc, dec!(25), dec!(1), 3000));

        // Only fills from 1500 to 2500
        let range_summary = pnl.calculate_pnl_in_range(1500, 2500, None);
        assert_eq!(range_summary.realized_pnl, dec!(50));
        assert_eq!(range_summary.fill_count, 1);
    }

    #[test]
    fn test_time_range() {
        let mut pnl = UserPnL::new("0x123".to_string());

        assert!(pnl.time_range().is_none());

        pnl.add_fill(make_fill(Asset::Btc, dec!(100), dec!(1), 2000));
        pnl.add_fill(make_fill(Asset::Eth, dec!(50), dec!(1), 1000));
        pnl.add_fill(make_fill(Asset::Sol, dec!(25), dec!(1), 3000));

        let (first, last) = pnl.time_range().unwrap();
        assert_eq!(first, 1000);
        assert_eq!(last, 3000);
    }

    #[test]
    fn test_all_fills_sorted() {
        let mut pnl = UserPnL::new("0x123".to_string());

        pnl.add_fill(make_fill(Asset::Btc, dec!(100), dec!(1), 3000));
        pnl.add_fill(make_fill(Asset::Eth, dec!(50), dec!(1), 1000));
        pnl.add_fill(make_fill(Asset::Sol, dec!(25), dec!(1), 2000));

        let fills = pnl.all_fills();
        assert_eq!(fills[0].timestamp_ms, 1000);
        assert_eq!(fills[1].timestamp_ms, 2000);
        assert_eq!(fills[2].timestamp_ms, 3000);
    }
}
