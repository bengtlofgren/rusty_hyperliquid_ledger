//! Type converters from hypersdk types to hl-types.
//!
//! This module provides conversion functions to transform raw API types
//! from hypersdk into the domain types defined in hl-types.

use hl_ingestion::Fill as HyperstkFill;
use hl_ingestion::Side as HyperstkSide;
use hl_types::{Asset, Side, UserFill};

/// Convert a hypersdk Fill to our UserFill type.
///
/// # Arguments
///
/// * `fill` - The hypersdk Fill from the API
///
/// # Returns
///
/// A `UserFill` with the same data in our domain model.
pub fn convert_fill(fill: &HyperstkFill) -> UserFill {
    UserFill {
        asset: Asset::from_symbol(&fill.coin),
        timestamp_ms: fill.time,
        price: fill.px,
        size: fill.sz,
        side: convert_side(&fill.side),
        fee: fill.fee,
        closed_pnl: fill.closed_pnl,
        trade_id: fill.tid,
        order_id: fill.oid,
        crossed: fill.crossed,
        direction: fill.dir.clone(),
    }
}

/// Convert multiple hypersdk Fills to UserFills.
pub fn convert_fills(fills: &[HyperstkFill]) -> Vec<UserFill> {
    fills.iter().map(convert_fill).collect()
}

/// Convert hypersdk Side to our Side type.
fn convert_side(side: &HyperstkSide) -> Side {
    match side {
        HyperstkSide::Bid => Side::Buy,
        HyperstkSide::Ask => Side::Sell,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_hypersdk_fill() -> HyperstkFill {
        // Create a mock Fill for testing
        // Note: This would require access to hypersdk's Fill constructor
        // For now we'll test the conversion logic conceptually
        HyperstkFill {
            coin: "BTC".to_string(),
            px: dec!(50000),
            sz: dec!(0.1),
            side: HyperstkSide::Bid,
            time: 1704067200000,
            start_position: dec!(0),
            dir: "Open Long".to_string(),
            closed_pnl: dec!(0),
            hash: "0x123".to_string(),
            oid: 12345,
            crossed: true,
            fee: dec!(5),
            tid: 67890,
            cloid: None,
            fee_token: "USDC".to_string(),
            liquidation: None,
        }
    }

    #[test]
    fn test_convert_fill() {
        let sdk_fill = make_hypersdk_fill();
        let user_fill = convert_fill(&sdk_fill);

        assert_eq!(user_fill.asset, Asset::Btc);
        assert_eq!(user_fill.timestamp_ms, 1704067200000);
        assert_eq!(user_fill.price, dec!(50000));
        assert_eq!(user_fill.size, dec!(0.1));
        assert!(matches!(user_fill.side, Side::Buy));
        assert_eq!(user_fill.fee, dec!(5));
        assert_eq!(user_fill.trade_id, 67890);
        assert_eq!(user_fill.order_id, 12345);
        assert!(user_fill.crossed);
    }

    #[test]
    fn test_convert_side() {
        assert!(matches!(convert_side(&HyperstkSide::Bid), Side::Buy));
        assert!(matches!(convert_side(&HyperstkSide::Ask), Side::Sell));
    }
}
