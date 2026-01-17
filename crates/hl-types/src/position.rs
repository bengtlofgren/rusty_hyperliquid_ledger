//! Position tracking types.
//!
//! This module provides the [`Position`] struct for tracking a user's
//! position in a specific asset, including entry price, size, and PnL.

use crate::Asset;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A user's position in a specific asset.
///
/// Represents the current state of a user's exposure to an asset,
/// including size, entry price, and unrealized PnL.
///
/// # Position Size Convention
///
/// - Positive size: Long position (profit when price goes up)
/// - Negative size: Short position (profit when price goes down)
/// - Zero size: No position
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// The user's address (hex string).
    pub user: String,

    /// The asset this position is for.
    pub asset: Asset,

    /// Position size (positive for long, negative for short).
    pub size: Decimal,

    /// Average entry price.
    /// None if no position (size == 0).
    pub entry_price: Option<Decimal>,

    /// Current mark price (if known).
    pub mark_price: Option<Decimal>,

    /// Unrealized PnL based on mark price.
    pub unrealized_pnl: Option<Decimal>,

    /// Total realized PnL from this position.
    pub realized_pnl: Decimal,

    /// Total fees paid for this position.
    pub total_fees: Decimal,

    /// Leverage used (if applicable).
    pub leverage: Option<u32>,

    /// Liquidation price (if applicable).
    pub liquidation_price: Option<Decimal>,

    /// Timestamp of last update (milliseconds since Unix epoch).
    pub last_updated_ms: u64,
}

impl Position {
    /// Create a new empty position for a user and asset.
    pub fn new(user: String, asset: Asset) -> Self {
        Self {
            user,
            asset,
            size: Decimal::ZERO,
            entry_price: None,
            mark_price: None,
            unrealized_pnl: None,
            realized_pnl: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            leverage: None,
            liquidation_price: None,
            last_updated_ms: 0,
        }
    }

    /// Check if this position is open (non-zero size).
    pub fn is_open(&self) -> bool {
        !self.size.is_zero()
    }

    /// Check if this is a long position.
    pub fn is_long(&self) -> bool {
        self.size > Decimal::ZERO
    }

    /// Check if this is a short position.
    pub fn is_short(&self) -> bool {
        self.size < Decimal::ZERO
    }

    /// Get the absolute size of the position.
    pub fn abs_size(&self) -> Decimal {
        self.size.abs()
    }

    /// Get the notional value of the position (size * entry_price).
    /// Returns None if no entry price is set.
    pub fn notional_value(&self) -> Option<Decimal> {
        self.entry_price.map(|price| self.size.abs() * price)
    }

    /// Calculate unrealized PnL given a mark price.
    ///
    /// For a long position: (mark_price - entry_price) * size
    /// For a short position: (entry_price - mark_price) * abs(size)
    pub fn calculate_unrealized_pnl(&self, mark_price: Decimal) -> Option<Decimal> {
        self.entry_price.map(|entry| (mark_price - entry) * self.size)
    }

    /// Get total PnL (realized + unrealized).
    pub fn total_pnl(&self) -> Decimal {
        self.realized_pnl + self.unrealized_pnl.unwrap_or(Decimal::ZERO)
    }

    /// Get net PnL after fees.
    pub fn net_pnl(&self) -> Decimal {
        self.total_pnl() - self.total_fees
    }

    /// Update the mark price and recalculate unrealized PnL.
    pub fn update_mark_price(&mut self, mark_price: Decimal) {
        self.mark_price = Some(mark_price);
        self.unrealized_pnl = self.calculate_unrealized_pnl(mark_price);
    }
}

impl Default for Position {
    fn default() -> Self {
        Self {
            user: String::new(),
            asset: Asset::Other("UNKNOWN".to_string()),
            size: Decimal::ZERO,
            entry_price: None,
            mark_price: None,
            unrealized_pnl: None,
            realized_pnl: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            leverage: None,
            liquidation_price: None,
            last_updated_ms: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_new_position() {
        let pos = Position::new("0x123".to_string(), Asset::Btc);
        assert!(!pos.is_open());
        assert!(!pos.is_long());
        assert!(!pos.is_short());
    }

    #[test]
    fn test_long_position() {
        let mut pos = Position::new("0x123".to_string(), Asset::Btc);
        pos.size = dec!(1.5);
        pos.entry_price = Some(dec!(40000));

        assert!(pos.is_open());
        assert!(pos.is_long());
        assert!(!pos.is_short());
        assert_eq!(pos.abs_size(), dec!(1.5));
        assert_eq!(pos.notional_value(), Some(dec!(60000)));
    }

    #[test]
    fn test_short_position() {
        let mut pos = Position::new("0x123".to_string(), Asset::Eth);
        pos.size = dec!(-10);
        pos.entry_price = Some(dec!(2000));

        assert!(pos.is_open());
        assert!(!pos.is_long());
        assert!(pos.is_short());
        assert_eq!(pos.abs_size(), dec!(10));
        assert_eq!(pos.notional_value(), Some(dec!(20000)));
    }

    #[test]
    fn test_unrealized_pnl_long() {
        let mut pos = Position::new("0x123".to_string(), Asset::Btc);
        pos.size = dec!(1);
        pos.entry_price = Some(dec!(40000));

        // Price goes up - profit for long
        assert_eq!(pos.calculate_unrealized_pnl(dec!(42000)), Some(dec!(2000)));
        // Price goes down - loss for long
        assert_eq!(pos.calculate_unrealized_pnl(dec!(38000)), Some(dec!(-2000)));
    }

    #[test]
    fn test_unrealized_pnl_short() {
        let mut pos = Position::new("0x123".to_string(), Asset::Btc);
        pos.size = dec!(-1);
        pos.entry_price = Some(dec!(40000));

        // Price goes up - loss for short
        assert_eq!(pos.calculate_unrealized_pnl(dec!(42000)), Some(dec!(-2000)));
        // Price goes down - profit for short
        assert_eq!(pos.calculate_unrealized_pnl(dec!(38000)), Some(dec!(2000)));
    }

    #[test]
    fn test_update_mark_price() {
        let mut pos = Position::new("0x123".to_string(), Asset::Btc);
        pos.size = dec!(1);
        pos.entry_price = Some(dec!(40000));

        pos.update_mark_price(dec!(45000));

        assert_eq!(pos.mark_price, Some(dec!(45000)));
        assert_eq!(pos.unrealized_pnl, Some(dec!(5000)));
    }

    #[test]
    fn test_total_pnl() {
        let mut pos = Position::new("0x123".to_string(), Asset::Btc);
        pos.realized_pnl = dec!(1000);
        pos.unrealized_pnl = Some(dec!(500));
        pos.total_fees = dec!(50);

        assert_eq!(pos.total_pnl(), dec!(1500));
        assert_eq!(pos.net_pnl(), dec!(1450));
    }
}
