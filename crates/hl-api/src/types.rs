//! API request and response types.

use hl_indexer::leaderboard::LeaderboardMetric;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Query parameters for fetching user trades/fills.
#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    /// User address (required).
    pub user: String,
    /// Start time in milliseconds since epoch.
    pub from_ms: Option<i64>,
    /// End time in milliseconds since epoch.
    pub to_ms: Option<i64>,
    /// Filter by asset symbol (e.g., "BTC", "ETH").
    pub asset: Option<String>,
    /// Maximum number of results to return.
    pub limit: Option<usize>,
}

/// Query parameters for fetching PnL.
#[derive(Debug, Deserialize)]
pub struct PnLQuery {
    /// User address (required).
    pub user: String,
    /// Start time in milliseconds since epoch.
    pub from_ms: Option<i64>,
    /// End time in milliseconds since epoch.
    pub to_ms: Option<i64>,
    /// Filter by asset symbols (comma-separated).
    pub assets: Option<String>,
}

/// A single trade/fill in the API response.
#[derive(Debug, Serialize)]
pub struct TradeResponse {
    /// Asset symbol.
    pub asset: String,
    /// Trade timestamp (milliseconds since epoch).
    pub timestamp_ms: u64,
    /// Execution price.
    pub price: Decimal,
    /// Trade size.
    pub size: Decimal,
    /// Trade side: "buy" or "sell".
    pub side: String,
    /// Fee paid.
    pub fee: Decimal,
    /// Closed PnL from this trade.
    pub closed_pnl: Decimal,
    /// Unique trade ID.
    pub trade_id: u64,
    /// Order ID this fill belongs to.
    pub order_id: u64,
    /// Whether the order crossed the spread.
    pub crossed: bool,
    /// Direction description (e.g., "Open Long", "Close Short").
    pub direction: String,
}

impl From<hl_types::UserFill> for TradeResponse {
    fn from(fill: hl_types::UserFill) -> Self {
        Self {
            asset: fill.asset.symbol().to_string(),
            timestamp_ms: fill.timestamp_ms,
            price: fill.price,
            size: fill.size,
            side: match fill.side {
                hl_types::Side::Buy => "buy".to_string(),
                hl_types::Side::Sell => "sell".to_string(),
            },
            fee: fill.fee,
            closed_pnl: fill.closed_pnl,
            trade_id: fill.trade_id,
            order_id: fill.order_id,
            crossed: fill.crossed,
            direction: fill.direction,
        }
    }
}

/// Response containing a list of trades.
#[derive(Debug, Serialize)]
pub struct TradesResponse {
    /// List of trades.
    pub trades: Vec<TradeResponse>,
    /// Total count (may be limited by query).
    pub count: usize,
    /// Whether more results exist beyond the limit.
    pub has_more: bool,
}

/// Per-asset PnL breakdown in the API response.
#[derive(Debug, Serialize)]
pub struct AssetPnLResponse {
    /// Asset symbol.
    pub asset: String,
    /// Realized PnL for this asset.
    pub realized_pnl: Decimal,
    /// Total fees paid for this asset.
    pub fees: Decimal,
    /// Net PnL (realized - fees).
    pub net_pnl: Decimal,
    /// Number of fills.
    pub fill_count: usize,
    /// Total volume traded.
    pub volume: Decimal,
}

impl From<&hl_types::AssetPnL> for AssetPnLResponse {
    fn from(pnl: &hl_types::AssetPnL) -> Self {
        Self {
            asset: pnl.asset.symbol().to_string(),
            realized_pnl: pnl.realized_pnl,
            fees: pnl.fees,
            net_pnl: pnl.net_pnl,
            fill_count: pnl.fill_count,
            volume: pnl.volume,
        }
    }
}

/// PnL summary response.
#[derive(Debug, Serialize)]
pub struct PnLResponse {
    /// User address.
    pub user: String,
    /// Total realized PnL.
    pub realized_pnl: Decimal,
    /// Total fees paid.
    pub total_fees: Decimal,
    /// Net PnL (realized - fees).
    pub net_pnl: Decimal,
    /// Total number of fills.
    pub fill_count: usize,
    /// Per-asset breakdown.
    pub by_asset: Vec<AssetPnLResponse>,
    /// Query time range start (if specified).
    pub from_ms: Option<i64>,
    /// Query time range end (if specified).
    pub to_ms: Option<i64>,
}

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Service status.
    pub status: String,
    /// Service version.
    pub version: String,
}

/// Query parameters for the leaderboard endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardQuery {
    /// Filter by coin/asset symbol (e.g., "BTC", "ETH").
    pub coin: Option<String>,
    /// Start time in milliseconds since epoch.
    pub from_ms: Option<i64>,
    /// End time in milliseconds since epoch.
    pub to_ms: Option<i64>,
    /// Metric to rank by: "volume", "pnl", or "returnPct".
    #[serde(default = "default_metric")]
    pub metric: String,
    /// Filter to only show users who used the builder.
    #[serde(default)]
    pub builder_only: bool,
    /// Maximum start capital for return percentage calculation.
    pub max_start_capital: Option<Decimal>,
}

fn default_metric() -> String {
    "volume".to_string()
}

impl LeaderboardQuery {
    /// Parse the metric string to LeaderboardMetric.
    pub fn parse_metric(&self) -> Option<LeaderboardMetric> {
        LeaderboardMetric::from_str(&self.metric)
    }
}

/// A single entry in the leaderboard.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntryResponse {
    /// Rank (1-indexed).
    pub rank: usize,
    /// User address.
    pub user: String,
    /// Value of the ranking metric.
    pub metric_value: Decimal,
    /// Total trading volume.
    pub volume: Decimal,
    /// Realized PnL.
    pub realized_pnl: Decimal,
    /// Return percentage (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_pct: Option<Decimal>,
    /// Number of trades.
    pub trade_count: usize,
    /// Number of fills that went through the builder.
    pub builder_fill_count: usize,
    /// Whether the user is tainted (has non-builder fills during open positions).
    pub tainted: bool,
}

impl From<hl_indexer::leaderboard::LeaderboardEntry> for LeaderboardEntryResponse {
    fn from(entry: hl_indexer::leaderboard::LeaderboardEntry) -> Self {
        Self {
            rank: entry.rank,
            user: entry.user,
            metric_value: entry.metric_value,
            volume: entry.volume,
            realized_pnl: entry.realized_pnl,
            return_pct: entry.return_pct,
            trade_count: entry.trade_count,
            builder_fill_count: entry.builder_fill_count,
            tainted: entry.tainted,
        }
    }
}

/// Leaderboard response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardResponse {
    /// Leaderboard entries.
    pub entries: Vec<LeaderboardEntryResponse>,
    /// Metric used for ranking.
    pub metric: String,
    /// Time range start (if specified).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_ms: Option<i64>,
    /// Time range end (if specified).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_ms: Option<i64>,
    /// Coin filter (if specified).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coin: Option<String>,
    /// Whether builder-only mode is enabled.
    pub builder_only: bool,
    /// Total number of users in the competition.
    pub total_users: usize,
    /// Number of users after taint filtering (if builder_only).
    pub filtered_users: usize,
}
