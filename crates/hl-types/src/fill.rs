//! Fill (trade execution) types.
//!
//! This module provides [`UserFill`], a representation of a trade execution
//! that occurred for a user. Fills are the fundamental building block for
//! calculating PnL and reconstructing position history.

use crate::Asset;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Trade execution side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    /// Buy (long) order.
    #[serde(alias = "B", alias = "buy", alias = "Bid")]
    Buy,
    /// Sell (short) order.
    #[serde(alias = "S", alias = "sell", alias = "Ask")]
    Sell,
}

impl Side {
    /// Returns true if this is a buy order.
    pub fn is_buy(&self) -> bool {
        matches!(self, Side::Buy)
    }

    /// Returns the sign for position calculations (+1 for buy, -1 for sell).
    pub fn sign(&self) -> Decimal {
        match self {
            Side::Buy => Decimal::ONE,
            Side::Sell => -Decimal::ONE,
        }
    }
}

/// A fill (trade execution) for a user.
///
/// This struct captures all the information about a single trade execution,
/// including price, size, fees, and realized PnL.
///
/// # Fields
///
/// - `asset`: The traded asset
/// - `timestamp`: When the fill occurred
/// - `price`: Execution price
/// - `size`: Execution size (always positive)
/// - `side`: Buy or sell
/// - `fee`: Trading fee paid
/// - `closed_pnl`: Realized PnL from closing a position
/// - `trade_id`: Unique identifier for this trade
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserFill {
    /// The asset that was traded.
    pub asset: Asset,

    /// Timestamp of the fill (milliseconds since Unix epoch).
    pub timestamp_ms: u64,

    /// Execution price.
    pub price: Decimal,

    /// Fill size (always positive, direction indicated by side).
    pub size: Decimal,

    /// Order side (buy/sell).
    pub side: Side,

    /// Trading fee paid.
    pub fee: Decimal,

    /// Realized PnL from closing a position.
    /// This is non-zero when the fill closes or reduces an existing position.
    pub closed_pnl: Decimal,

    /// Unique trade identifier.
    pub trade_id: u64,

    /// Order ID this fill belongs to.
    pub order_id: u64,

    /// Whether this fill was a taker (crossed the spread).
    pub crossed: bool,

    /// Direction description (e.g., "Open Long", "Close Short").
    pub direction: String,
}

impl UserFill {
    /// Get the timestamp as a DateTime.
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp_millis(self.timestamp_ms as i64)
    }

    /// Calculate the notional value of this fill (price * size).
    pub fn notional_value(&self) -> Decimal {
        self.price * self.size
    }

    /// Get the signed size based on side (+size for buy, -size for sell).
    pub fn signed_size(&self) -> Decimal {
        self.size * self.side.sign()
    }

    /// Get the net PnL contribution (closed_pnl - fee).
    pub fn net_pnl(&self) -> Decimal {
        self.closed_pnl - self.fee
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_fill() -> UserFill {
        UserFill {
            asset: Asset::Btc,
            timestamp_ms: 1704067200000,
            price: dec!(42000),
            size: dec!(0.1),
            side: Side::Buy,
            fee: dec!(4.2),
            closed_pnl: dec!(0),
            trade_id: 12345,
            order_id: 67890,
            crossed: true,
            direction: "Open Long".to_string(),
        }
    }

    #[test]
    fn test_notional_value() {
        let fill = sample_fill();
        assert_eq!(fill.notional_value(), dec!(4200));
    }

    #[test]
    fn test_signed_size() {
        let mut fill = sample_fill();
        assert_eq!(fill.signed_size(), dec!(0.1));

        fill.side = Side::Sell;
        assert_eq!(fill.signed_size(), dec!(-0.1));
    }

    #[test]
    fn test_net_pnl() {
        let mut fill = sample_fill();
        fill.closed_pnl = dec!(100);
        fill.fee = dec!(4.2);
        assert_eq!(fill.net_pnl(), dec!(95.8));
    }

    #[test]
    fn test_timestamp() {
        let fill = sample_fill();
        let ts = fill.timestamp().unwrap();
        assert_eq!(ts.timestamp_millis(), 1704067200000);
    }
}
