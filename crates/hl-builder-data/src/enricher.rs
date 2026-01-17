//! Fill enrichment service for matching builder fills with regular fills.
//!
//! Since the builder fills CSV doesn't include a trade ID (tid), we match
//! fills using a composite key of (user, coin, time, size, price, side).

use crate::types::BuilderFill;
use hl_types::{Asset, UserFill};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Key for matching fills between builder data and regular fills.
///
/// Since builder fills don't have a trade ID, we use a composite key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FillKey {
    user: String,
    coin: String,
    /// Timestamp in milliseconds, rounded to nearest second for fuzzy matching.
    time_sec: i64,
    /// Size as string for exact decimal comparison.
    size: String,
    /// Price as string for exact decimal comparison.
    price: String,
    /// True for buy/bid, false for sell/ask.
    is_buy: bool,
}

impl FillKey {
    /// Create a key from a builder fill.
    fn from_builder_fill(fill: &BuilderFill) -> Self {
        Self {
            user: fill.user.to_lowercase(),
            coin: fill.asset.symbol().to_uppercase(),
            time_sec: fill.time.timestamp(),
            size: fill.size.to_string(),
            price: fill.price.to_string(),
            is_buy: fill.side.is_buy(),
        }
    }

    /// Create a key from a user fill.
    fn from_user_fill(fill: &UserFill, user: &str) -> Self {
        Self {
            user: user.to_lowercase(),
            coin: fill.asset.symbol().to_uppercase(),
            time_sec: (fill.timestamp_ms / 1000) as i64,
            size: fill.size.to_string(),
            price: fill.price.to_string(),
            is_buy: matches!(fill.side, hl_types::Side::Buy),
        }
    }
}

/// Service for enriching regular fills with builder attribution data.
///
/// # Matching Strategy
///
/// Since builder fills don't include a trade ID, matching is done using:
/// 1. User address
/// 2. Asset (coin)
/// 3. Timestamp (rounded to second)
/// 4. Size (exact match)
/// 5. Price (exact match)
/// 6. Side (buy/sell)
///
/// # Example
///
/// ```rust
/// use hl_builder_data::{BuilderFill, FillEnricher};
///
/// let builder_fills: Vec<BuilderFill> = vec![/* ... */];
/// let enricher = FillEnricher::new(builder_fills);
///
/// // Check if a fill was from the tracked builder
/// // let is_builder = enricher.is_builder_fill(&user_fill, "0x...");
/// ```
pub struct FillEnricher {
    /// Builder fills indexed by composite key.
    fills_by_key: HashMap<FillKey, BuilderFill>,

    /// Total number of builder fills loaded.
    total_fills: usize,
}

impl FillEnricher {
    /// Create a new enricher from a list of builder fills.
    pub fn new(fills: Vec<BuilderFill>) -> Self {
        let total_fills = fills.len();
        let mut fills_by_key = HashMap::with_capacity(fills.len());

        for fill in fills {
            let key = FillKey::from_builder_fill(&fill);
            fills_by_key.insert(key, fill);
        }

        Self {
            fills_by_key,
            total_fills,
        }
    }

    /// Get the total number of builder fills loaded.
    pub fn total_fills(&self) -> usize {
        self.total_fills
    }

    /// Check if a fill was attributed to the tracked builder.
    ///
    /// # Arguments
    ///
    /// * `fill` - The user fill to check
    /// * `user` - The user address (for matching)
    pub fn is_builder_fill(&self, fill: &UserFill, user: &str) -> bool {
        let key = FillKey::from_user_fill(fill, user);
        self.fills_by_key.contains_key(&key)
    }

    /// Get the builder fill data if this fill was from the builder.
    ///
    /// # Arguments
    ///
    /// * `fill` - The user fill to look up
    /// * `user` - The user address (for matching)
    pub fn get_builder_fill(&self, fill: &UserFill, user: &str) -> Option<&BuilderFill> {
        let key = FillKey::from_user_fill(fill, user);
        self.fills_by_key.get(&key)
    }

    /// Get the builder fee for a fill if it exists.
    ///
    /// # Arguments
    ///
    /// * `fill` - The user fill to look up
    /// * `user` - The user address (for matching)
    pub fn get_builder_fee(&self, fill: &UserFill, user: &str) -> Option<Decimal> {
        self.get_builder_fill(fill, user)
            .map(|bf| bf.builder_fee)
    }

    /// Get all builder fills for a specific user.
    pub fn fills_for_user(&self, user: &str) -> Vec<&BuilderFill> {
        let user_lower = user.to_lowercase();
        self.fills_by_key
            .values()
            .filter(|f| f.user.to_lowercase() == user_lower)
            .collect()
    }

    /// Get all builder fills for a specific asset.
    pub fn fills_for_asset(&self, asset: &Asset) -> Vec<&BuilderFill> {
        let symbol = asset.symbol().to_uppercase();
        self.fills_by_key
            .values()
            .filter(|f| f.asset.symbol().to_uppercase() == symbol)
            .collect()
    }

    /// Calculate total builder fees collected.
    pub fn total_builder_fees(&self) -> Decimal {
        self.fills_by_key
            .values()
            .map(|f| f.builder_fee)
            .sum()
    }

    /// Calculate total volume (sum of notional values).
    pub fn total_volume(&self) -> Decimal {
        self.fills_by_key
            .values()
            .map(|f| f.notional_value())
            .sum()
    }
}

impl Default for FillEnricher {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BuilderFillSide;
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;

    fn make_builder_fill(
        user: &str,
        coin: &str,
        time_sec: i64,
        price: Decimal,
        size: Decimal,
        is_buy: bool,
        builder_fee: Decimal,
    ) -> BuilderFill {
        BuilderFill {
            time: Utc.timestamp_opt(time_sec, 0).unwrap(),
            user: user.to_string(),
            asset: Asset::from_symbol(coin),
            side: if is_buy {
                BuilderFillSide::Bid
            } else {
                BuilderFillSide::Ask
            },
            price,
            size,
            crossed: false,
            special_trade_type: "Na".to_string(),
            time_in_force: "Gtc".to_string(),
            is_trigger: false,
            counterparty: "0x0".to_string(),
            closed_pnl: Decimal::ZERO,
            twap_id: 0,
            builder_fee,
        }
    }

    fn make_user_fill(
        coin: &str,
        timestamp_ms: u64,
        price: Decimal,
        size: Decimal,
        is_buy: bool,
    ) -> UserFill {
        UserFill {
            asset: Asset::from_symbol(coin),
            timestamp_ms,
            price,
            size,
            side: if is_buy {
                hl_types::Side::Buy
            } else {
                hl_types::Side::Sell
            },
            fee: dec!(0.1),
            closed_pnl: Decimal::ZERO,
            trade_id: 12345,
            order_id: 67890,
            crossed: false,
            direction: "Open Long".to_string(),
        }
    }

    #[test]
    fn test_enricher_basic() {
        let builder_fills = vec![
            make_builder_fill("0xABC", "BTC", 1000, dec!(50000), dec!(0.1), true, dec!(0.5)),
            make_builder_fill("0xABC", "ETH", 2000, dec!(3000), dec!(1.0), false, dec!(0.3)),
        ];

        let enricher = FillEnricher::new(builder_fills);

        assert_eq!(enricher.total_fills(), 2);
    }

    #[test]
    fn test_is_builder_fill() {
        let builder_fills = vec![make_builder_fill(
            "0xabc",
            "BTC",
            1000,
            dec!(50000),
            dec!(0.1),
            true,
            dec!(0.5),
        )];

        let enricher = FillEnricher::new(builder_fills);

        // Matching fill (timestamp in ms = 1000 * 1000 = 1000000)
        let user_fill = make_user_fill("BTC", 1000000, dec!(50000), dec!(0.1), true);
        assert!(enricher.is_builder_fill(&user_fill, "0xABC"));

        // Non-matching fill (different price)
        let user_fill_diff = make_user_fill("BTC", 1000000, dec!(51000), dec!(0.1), true);
        assert!(!enricher.is_builder_fill(&user_fill_diff, "0xABC"));
    }

    #[test]
    fn test_get_builder_fee() {
        let builder_fills = vec![make_builder_fill(
            "0xabc",
            "SOL",
            5000,
            dec!(135.88),
            dec!(0.23),
            true,
            dec!(0.003125),
        )];

        let enricher = FillEnricher::new(builder_fills);

        let user_fill = make_user_fill("SOL", 5000000, dec!(135.88), dec!(0.23), true);
        let fee = enricher.get_builder_fee(&user_fill, "0xABC");

        assert_eq!(fee, Some(dec!(0.003125)));
    }

    #[test]
    fn test_total_fees_and_volume() {
        let builder_fills = vec![
            make_builder_fill("0xabc", "BTC", 1000, dec!(50000), dec!(0.1), true, dec!(0.5)),
            make_builder_fill("0xabc", "ETH", 2000, dec!(3000), dec!(1.0), false, dec!(0.3)),
        ];

        let enricher = FillEnricher::new(builder_fills);

        assert_eq!(enricher.total_builder_fees(), dec!(0.8));
        // 50000 * 0.1 + 3000 * 1.0 = 5000 + 3000 = 8000
        assert_eq!(enricher.total_volume(), dec!(8000));
    }
}
