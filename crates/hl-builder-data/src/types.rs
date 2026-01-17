//! Types for builder fill data.

use chrono::{DateTime, Utc};
use hl_types::Asset;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};

/// A fill attributed to a specific builder.
///
/// This represents a single trade execution that was routed through
/// a builder's order flow.
#[derive(Debug, Clone, PartialEq)]
pub struct BuilderFill {
    /// Timestamp of the fill.
    pub time: DateTime<Utc>,

    /// User address who made the trade.
    pub user: String,

    /// Asset traded.
    pub asset: Asset,

    /// Order side.
    pub side: BuilderFillSide,

    /// Execution price.
    pub price: Decimal,

    /// Fill size.
    pub size: Decimal,

    /// Whether this was a taker order (crossed the spread).
    pub crossed: bool,

    /// Special trade type (e.g., "Na" for normal).
    pub special_trade_type: String,

    /// Time in force (e.g., "Alo", "Gtc").
    pub time_in_force: String,

    /// Whether this was a trigger order.
    pub is_trigger: bool,

    /// Counterparty address.
    pub counterparty: String,

    /// Realized PnL from closing a position.
    pub closed_pnl: Decimal,

    /// TWAP order ID (0 if not a TWAP order).
    pub twap_id: u64,

    /// Builder fee collected.
    pub builder_fee: Decimal,
}

/// Order side for builder fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuilderFillSide {
    /// Buy order (Bid).
    Bid,
    /// Sell order (Ask).
    Ask,
}

impl BuilderFillSide {
    /// Parse from CSV string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Bid" => Some(BuilderFillSide::Bid),
            "Ask" => Some(BuilderFillSide::Ask),
            _ => None,
        }
    }

    /// Check if this is a buy order.
    pub fn is_buy(&self) -> bool {
        matches!(self, BuilderFillSide::Bid)
    }
}

/// Raw CSV record for deserialization.
/// Maps directly to the CSV columns.
#[derive(Debug, Deserialize)]
pub(crate) struct BuilderFillRecord {
    #[serde(deserialize_with = "deserialize_datetime")]
    pub time: DateTime<Utc>,
    pub user: String,
    pub coin: String,
    pub side: String,
    #[serde(deserialize_with = "deserialize_decimal")]
    pub px: Decimal,
    #[serde(deserialize_with = "deserialize_decimal")]
    pub sz: Decimal,
    #[serde(deserialize_with = "deserialize_bool")]
    pub crossed: bool,
    pub special_trade_type: String,
    pub tif: String,
    #[serde(deserialize_with = "deserialize_bool")]
    pub is_trigger: bool,
    pub counterparty: String,
    #[serde(deserialize_with = "deserialize_decimal")]
    pub closed_pnl: Decimal,
    pub twap_id: u64,
    #[serde(deserialize_with = "deserialize_decimal")]
    pub builder_fee: Decimal,
}

impl TryFrom<BuilderFillRecord> for BuilderFill {
    type Error = String;

    fn try_from(record: BuilderFillRecord) -> Result<Self, Self::Error> {
        let side = BuilderFillSide::from_str(&record.side)
            .ok_or_else(|| format!("invalid side: {}", record.side))?;

        Ok(BuilderFill {
            time: record.time,
            user: record.user,
            asset: Asset::from_symbol(&record.coin),
            side,
            price: record.px,
            size: record.sz,
            crossed: record.crossed,
            special_trade_type: record.special_trade_type,
            time_in_force: record.tif,
            is_trigger: record.is_trigger,
            counterparty: record.counterparty,
            closed_pnl: record.closed_pnl,
            twap_id: record.twap_id,
            builder_fee: record.builder_fee,
        })
    }
}

/// Deserialize ISO 8601 datetime.
fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(serde::de::Error::custom)
}

/// Deserialize decimal from string.
fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<Decimal>().map_err(serde::de::Error::custom)
}

/// Deserialize bool from string ("true"/"false").
fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(serde::de::Error::custom(format!("invalid bool: {}", s))),
    }
}

impl BuilderFill {
    /// Get the notional value of this fill (price * size).
    pub fn notional_value(&self) -> Decimal {
        self.price * self.size
    }

    /// Get timestamp as milliseconds since epoch.
    pub fn timestamp_ms(&self) -> i64 {
        self.time.timestamp_millis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_fill_side() {
        assert_eq!(BuilderFillSide::from_str("Bid"), Some(BuilderFillSide::Bid));
        assert_eq!(BuilderFillSide::from_str("Ask"), Some(BuilderFillSide::Ask));
        assert_eq!(BuilderFillSide::from_str("invalid"), None);

        assert!(BuilderFillSide::Bid.is_buy());
        assert!(!BuilderFillSide::Ask.is_buy());
    }
}
