//! Taint detection for builder-based competition enforcement.
//!
//! This module tracks position lifecycles and detects when fills occur
//! through channels other than the designated builder.
//!
//! A user is considered "tainted" if any fill during an open position
//! did not go through the target builder.

use hl_types::{Asset, Side, UserFill};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Result of analyzing a user's fills for taint.
#[derive(Debug, Clone)]
pub struct TaintAnalysisResult {
    /// Whether the user is tainted (had non-builder fills while in a position).
    pub tainted: bool,

    /// Assets that had tainted fills.
    pub tainted_assets: Vec<Asset>,

    /// Total number of fills analyzed.
    pub total_fills: usize,

    /// Number of fills that went through the builder.
    pub builder_fills: usize,

    /// Number of non-builder fills while in a position.
    pub tainted_fills: usize,

    /// First timestamp where taint was detected (if any).
    pub first_taint_timestamp_ms: Option<u64>,
}

impl Default for TaintAnalysisResult {
    fn default() -> Self {
        Self {
            tainted: false,
            tainted_assets: Vec::new(),
            total_fills: 0,
            builder_fills: 0,
            tainted_fills: 0,
            first_taint_timestamp_ms: None,
        }
    }
}

/// Tracks position lifecycle per asset for taint detection.
///
/// Position lifecycle:
/// - 0 → non-zero: Position opened
/// - non-zero → non-zero: Position modified
/// - non-zero → 0: Position closed
///
/// Taint is detected when a non-builder fill occurs while a position is open.
#[derive(Debug, Default)]
pub struct PositionLifecycleTracker {
    /// Net position size per asset.
    positions: HashMap<Asset, Decimal>,

    /// Whether each asset has been tainted.
    tainted_assets: HashMap<Asset, bool>,

    /// First taint timestamp.
    first_taint_ms: Option<u64>,

    /// Counts.
    total_fills: usize,
    builder_fills: usize,
    tainted_fills: usize,
}

impl PositionLifecycleTracker {
    /// Create a new position lifecycle tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a fill and update position state.
    ///
    /// Returns `true` if this fill caused taint.
    pub fn process_fill(&mut self, fill: &UserFill, is_builder_fill: bool) -> bool {
        self.total_fills += 1;

        let current_position = *self.positions.get(&fill.asset).unwrap_or(&Decimal::ZERO);

        // Calculate signed size: positive for buy, negative for sell
        let signed_size = match fill.side {
            Side::Buy => fill.size,
            Side::Sell => -fill.size,
        };

        let new_position = current_position + signed_size;
        self.positions.insert(fill.asset.clone(), new_position);

        if is_builder_fill {
            self.builder_fills += 1;
            return false;
        }

        // Check if we're in a position (before or after this fill)
        // A fill is tainted if:
        // 1. We had a position before this fill, OR
        // 2. This fill opened a position (and wasn't a builder fill)
        let was_in_position = current_position != Decimal::ZERO;
        let is_in_position = new_position != Decimal::ZERO;

        if was_in_position || is_in_position {
            // Non-builder fill while in a position = tainted
            self.tainted_fills += 1;
            self.tainted_assets.insert(fill.asset.clone(), true);

            if self.first_taint_ms.is_none() {
                self.first_taint_ms = Some(fill.timestamp_ms);
            }

            return true;
        }

        false
    }

    /// Check if the user has any tainted assets.
    pub fn is_tainted(&self) -> bool {
        !self.tainted_assets.is_empty()
    }

    /// Get the analysis result.
    pub fn result(&self) -> TaintAnalysisResult {
        TaintAnalysisResult {
            tainted: self.is_tainted(),
            tainted_assets: self.tainted_assets.keys().cloned().collect(),
            total_fills: self.total_fills,
            builder_fills: self.builder_fills,
            tainted_fills: self.tainted_fills,
            first_taint_timestamp_ms: self.first_taint_ms,
        }
    }

    /// Get the current position for an asset.
    pub fn get_position(&self, asset: &Asset) -> Decimal {
        *self.positions.get(asset).unwrap_or(&Decimal::ZERO)
    }
}

/// Analyze fills for taint given a builder fill checker function.
///
/// Fills must be sorted by timestamp in ascending order for accurate
/// position lifecycle tracking.
pub fn analyze_user_taint<F>(fills: &[UserFill], is_builder_fill: F) -> TaintAnalysisResult
where
    F: Fn(&UserFill) -> bool,
{
    let mut tracker = PositionLifecycleTracker::new();

    // Sort fills by timestamp to ensure correct position lifecycle tracking
    let mut sorted_fills: Vec<&UserFill> = fills.iter().collect();
    sorted_fills.sort_by_key(|f| f.timestamp_ms);

    for fill in sorted_fills {
        tracker.process_fill(fill, is_builder_fill(fill));
    }

    tracker.result()
}

/// Simple taint analysis when you have a set of builder fill trade IDs.
pub fn analyze_user_taint_with_ids(
    fills: &[UserFill],
    builder_trade_ids: &std::collections::HashSet<u64>,
) -> TaintAnalysisResult {
    analyze_user_taint(fills, |fill| builder_trade_ids.contains(&fill.trade_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_fill(asset: Asset, side: Side, size: Decimal, timestamp_ms: u64, trade_id: u64) -> UserFill {
        UserFill {
            asset,
            timestamp_ms,
            price: dec!(100),
            size,
            side,
            fee: dec!(0.1),
            closed_pnl: Decimal::ZERO,
            trade_id,
            order_id: trade_id,
            crossed: true,
            direction: "Test".to_string(),
        }
    }

    #[test]
    fn test_no_fills_not_tainted() {
        let result = analyze_user_taint(&[], |_| true);
        assert!(!result.tainted);
        assert_eq!(result.total_fills, 0);
    }

    #[test]
    fn test_all_builder_fills_not_tainted() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(1), 1000, 1),
            make_fill(Asset::Btc, Side::Sell, dec!(1), 2000, 2),
        ];

        let result = analyze_user_taint(&fills, |_| true);
        assert!(!result.tainted);
        assert_eq!(result.total_fills, 2);
        assert_eq!(result.builder_fills, 2);
        assert_eq!(result.tainted_fills, 0);
    }

    #[test]
    fn test_non_builder_fill_opens_position_is_tainted() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(1), 1000, 1),
        ];

        // No builder fills
        let result = analyze_user_taint(&fills, |_| false);
        assert!(result.tainted);
        assert_eq!(result.tainted_fills, 1);
        assert_eq!(result.first_taint_timestamp_ms, Some(1000));
    }

    #[test]
    fn test_builder_open_non_builder_close_is_tainted() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(1), 1000, 1),  // Open via builder
            make_fill(Asset::Btc, Side::Sell, dec!(1), 2000, 2), // Close NOT via builder
        ];

        let result = analyze_user_taint(&fills, |f| f.trade_id == 1);
        assert!(result.tainted);
        assert_eq!(result.builder_fills, 1);
        assert_eq!(result.tainted_fills, 1);
        assert_eq!(result.first_taint_timestamp_ms, Some(2000));
    }

    #[test]
    fn test_non_builder_open_builder_close_is_tainted() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(1), 1000, 1),  // Open NOT via builder
            make_fill(Asset::Btc, Side::Sell, dec!(1), 2000, 2), // Close via builder
        ];

        let result = analyze_user_taint(&fills, |f| f.trade_id == 2);
        assert!(result.tainted);
        assert_eq!(result.builder_fills, 1);
        assert_eq!(result.tainted_fills, 1);
        assert_eq!(result.first_taint_timestamp_ms, Some(1000));
    }

    #[test]
    fn test_partial_position_modification_tainted() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(2), 1000, 1),  // Open 2 via builder
            make_fill(Asset::Btc, Side::Sell, dec!(1), 2000, 2), // Reduce to 1 NOT via builder
            make_fill(Asset::Btc, Side::Sell, dec!(1), 3000, 3), // Close via builder
        ];

        let result = analyze_user_taint(&fills, |f| f.trade_id != 2);
        assert!(result.tainted);
        assert_eq!(result.builder_fills, 2);
        assert_eq!(result.tainted_fills, 1);
    }

    #[test]
    fn test_multiple_assets_independent() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(1), 1000, 1),  // BTC open via builder
            make_fill(Asset::Eth, Side::Buy, dec!(1), 1500, 2),  // ETH open NOT via builder
            make_fill(Asset::Btc, Side::Sell, dec!(1), 2000, 3), // BTC close via builder
            make_fill(Asset::Eth, Side::Sell, dec!(1), 2500, 4), // ETH close via builder
        ];

        let result = analyze_user_taint(&fills, |f| f.trade_id != 2);
        assert!(result.tainted);
        assert!(result.tainted_assets.contains(&Asset::Eth));
        assert!(!result.tainted_assets.contains(&Asset::Btc));
    }

    #[test]
    fn test_position_tracker_get_position() {
        let mut tracker = PositionLifecycleTracker::new();

        let fill1 = make_fill(Asset::Btc, Side::Buy, dec!(2), 1000, 1);
        let fill2 = make_fill(Asset::Btc, Side::Sell, dec!(0.5), 2000, 2);

        tracker.process_fill(&fill1, true);
        assert_eq!(tracker.get_position(&Asset::Btc), dec!(2));

        tracker.process_fill(&fill2, true);
        assert_eq!(tracker.get_position(&Asset::Btc), dec!(1.5));
    }

    #[test]
    fn test_fills_out_of_order_sorted_correctly() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Sell, dec!(1), 2000, 2), // Close (out of order)
            make_fill(Asset::Btc, Side::Buy, dec!(1), 1000, 1),  // Open (out of order)
        ];

        // Both are builder fills - should not be tainted
        let result = analyze_user_taint(&fills, |_| true);
        assert!(!result.tainted);
    }

    #[test]
    fn test_analyze_with_trade_ids() {
        use std::collections::HashSet;

        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(1), 1000, 1),
            make_fill(Asset::Btc, Side::Sell, dec!(1), 2000, 2),
        ];

        let builder_ids: HashSet<u64> = [1].into_iter().collect();
        let result = analyze_user_taint_with_ids(&fills, &builder_ids);

        assert!(result.tainted);
        assert_eq!(result.builder_fills, 1);
        assert_eq!(result.tainted_fills, 1);
    }
}
