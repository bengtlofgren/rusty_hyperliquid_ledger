//! Leaderboard calculation for trading competitions.
//!
//! This module provides functionality to calculate and rank users based on
//! various trading metrics like volume, PnL, and return percentage.

use crate::error::IndexerError;
use crate::taint::{analyze_user_taint, TaintAnalysisResult};
use crate::Indexer;
use futures::future::join_all;
use hl_types::{Asset, UserFill};
use rust_decimal::Decimal;
use std::cmp::Ordering;

/// Metric to rank the leaderboard by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderboardMetric {
    /// Total trading volume.
    Volume,
    /// Realized PnL.
    Pnl,
    /// Return percentage (requires from_ms and max_start_capital).
    ReturnPct,
}

impl LeaderboardMetric {
    /// Parse from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "volume" => Some(Self::Volume),
            "pnl" => Some(Self::Pnl),
            "returnpct" | "return_pct" | "return" => Some(Self::ReturnPct),
            _ => None,
        }
    }

    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Volume => "volume",
            Self::Pnl => "pnl",
            Self::ReturnPct => "returnPct",
        }
    }
}

/// User statistics for leaderboard ranking.
#[derive(Debug, Clone)]
pub struct UserStats {
    /// User address.
    pub user: String,

    /// Total trading volume.
    pub volume: Decimal,

    /// Realized PnL (closed_pnl - fees).
    pub realized_pnl: Decimal,

    /// Return percentage.
    pub return_pct: Option<Decimal>,

    /// Number of trades.
    pub trade_count: usize,

    /// Number of builder fills.
    pub builder_fill_count: usize,

    /// Taint analysis result.
    pub taint_result: TaintAnalysisResult,
}

impl UserStats {
    /// Get the metric value for ranking.
    pub fn get_metric_value(&self, metric: LeaderboardMetric) -> Decimal {
        match metric {
            LeaderboardMetric::Volume => self.volume,
            LeaderboardMetric::Pnl => self.realized_pnl,
            LeaderboardMetric::ReturnPct => self.return_pct.unwrap_or(Decimal::ZERO),
        }
    }
}

/// Ranked leaderboard entry.
#[derive(Debug, Clone)]
pub struct LeaderboardEntry {
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
    pub return_pct: Option<Decimal>,

    /// Number of trades.
    pub trade_count: usize,

    /// Number of builder fills.
    pub builder_fill_count: usize,

    /// Whether the user is tainted.
    pub tainted: bool,
}

/// Configuration for leaderboard calculation.
#[derive(Debug, Clone)]
pub struct LeaderboardConfig {
    /// Target builder address (lowercase).
    pub target_builder: Option<String>,

    /// Whether to filter out tainted users.
    pub builder_only: bool,

    /// Maximum start capital for return percentage calculation.
    pub max_start_capital: Option<Decimal>,

    /// Optional asset filter.
    pub coin: Option<String>,

    /// Time range start (milliseconds).
    pub from_ms: Option<i64>,

    /// Time range end (milliseconds).
    pub to_ms: Option<i64>,

    /// Metric to rank by.
    pub metric: LeaderboardMetric,
}

/// Trait for checking if a fill is a builder fill.
pub trait BuilderFillChecker: Send + Sync {
    /// Check if the given fill for the given user is a builder fill.
    fn is_builder_fill(&self, fill: &UserFill, user: &str) -> bool;
}

/// A no-op checker that always returns false (no builder fills).
pub struct NoBuilderChecker;

impl BuilderFillChecker for NoBuilderChecker {
    fn is_builder_fill(&self, _fill: &UserFill, _user: &str) -> bool {
        false
    }
}

#[cfg(feature = "builder-enrichment")]
mod enricher_checker {
    use super::*;
    use hl_builder_data::FillEnricher;

    /// Wrapper that implements `BuilderFillChecker` for `FillEnricher`.
    pub struct FillEnricherChecker {
        enricher: FillEnricher,
    }

    impl FillEnricherChecker {
        /// Create a new checker wrapping a `FillEnricher`.
        pub fn new(enricher: FillEnricher) -> Self {
            Self { enricher }
        }

        /// Get total fills in the enricher.
        pub fn total_fills(&self) -> usize {
            self.enricher.total_fills()
        }
    }

    impl BuilderFillChecker for FillEnricherChecker {
        fn is_builder_fill(&self, fill: &UserFill, user: &str) -> bool {
            self.enricher.is_builder_fill(fill, user)
        }
    }
}

#[cfg(feature = "builder-enrichment")]
pub use enricher_checker::FillEnricherChecker;

/// Calculate stats for a single user.
///
/// If `builder_only` is true, only builder fills are counted toward volume/PnL.
pub fn calculate_user_stats<C: BuilderFillChecker>(
    user: &str,
    fills: &[UserFill],
    builder_checker: &C,
    max_start_capital: Option<Decimal>,
    coin_filter: Option<&str>,
    builder_only: bool,
) -> UserStats {
    // Filter by coin if specified
    let fills: Vec<&UserFill> = if let Some(coin) = coin_filter {
        let target_asset = Asset::from_symbol(coin);
        fills.iter().filter(|f| f.asset == target_asset).collect()
    } else {
        fills.iter().collect()
    };

    // Calculate volume and PnL
    // When builder_only=true, only count builder fills toward metrics
    let mut volume = Decimal::ZERO;
    let mut realized_pnl = Decimal::ZERO;
    let mut builder_fill_count = 0;
    let mut counted_fills = 0;

    for fill in &fills {
        let is_builder = builder_checker.is_builder_fill(fill, user);
        if is_builder {
            builder_fill_count += 1;
        }

        // Only count this fill if we're not in builder_only mode, or if it's a builder fill
        if !builder_only || is_builder {
            volume += fill.price * fill.size;
            realized_pnl += fill.closed_pnl - fill.fee;
            counted_fills += 1;
        }
    }

    // Analyze taint using the builder checker (always on all fills)
    let owned_fills: Vec<UserFill> = fills.iter().map(|f| (*f).clone()).collect();
    let taint_result = analyze_user_taint(&owned_fills, |fill| {
        builder_checker.is_builder_fill(fill, user)
    });

    // Calculate return percentage
    let return_pct = max_start_capital.map(|capital| {
        if capital > Decimal::ZERO {
            (realized_pnl / capital) * Decimal::from(100)
        } else {
            Decimal::ZERO
        }
    });

    UserStats {
        user: user.to_string(),
        volume,
        realized_pnl,
        return_pct,
        trade_count: counted_fills,
        builder_fill_count,
        taint_result,
    }
}

/// Fetch fills and calculate stats for all users in parallel.
pub async fn calculate_leaderboard<C: BuilderFillChecker>(
    indexer: &Indexer,
    users: &[String],
    config: &LeaderboardConfig,
    builder_checker: &C,
) -> Result<Vec<UserStats>, IndexerError> {
    // Fetch fills for all users in parallel
    let fetch_futures: Vec<_> = users
        .iter()
        .map(|user| {
            let user = user.clone();
            let from_ms = config.from_ms;
            let to_ms = config.to_ms;
            async move {
                let fills = indexer.get_user_fills(&user, from_ms, to_ms).await;
                (user, fills)
            }
        })
        .collect();

    let results = join_all(fetch_futures).await;

    // Calculate stats for each user
    let mut stats = Vec::with_capacity(users.len());

    for (user, fills_result) in results {
        match fills_result {
            Ok(fills) => {
                let user_stats = calculate_user_stats(
                    &user,
                    &fills,
                    builder_checker,
                    config.max_start_capital,
                    config.coin.as_deref(),
                    config.builder_only,
                );
                stats.push(user_stats);
            }
            Err(e) => {
                tracing::warn!("Failed to fetch fills for user {}: {}", user, e);
                // Include user with zero stats rather than failing entirely
                stats.push(UserStats {
                    user,
                    volume: Decimal::ZERO,
                    realized_pnl: Decimal::ZERO,
                    return_pct: config.max_start_capital.map(|_| Decimal::ZERO),
                    trade_count: 0,
                    builder_fill_count: 0,
                    taint_result: TaintAnalysisResult::default(),
                });
            }
        }
    }

    Ok(stats)
}

/// Rank the leaderboard entries by metric.
///
/// Note: When `builder_only=true`, filtering happens at calculation time (only builder fills
/// are counted toward metrics), not at ranking time. All users are included in the results,
/// but those without builder fills will have zero metrics.
pub fn rank_leaderboard(
    stats: Vec<UserStats>,
    metric: LeaderboardMetric,
    _builder_only: bool,
) -> Vec<LeaderboardEntry> {
    let mut sorted = stats;

    // Sort by metric value (descending)
    sorted.sort_by(|a, b| {
        let a_val = a.get_metric_value(metric);
        let b_val = b.get_metric_value(metric);
        b_val.partial_cmp(&a_val).unwrap_or(Ordering::Equal)
    });

    // Convert to ranked entries
    sorted
        .into_iter()
        .enumerate()
        .map(|(idx, stats)| {
            let metric_value = stats.get_metric_value(metric);
            LeaderboardEntry {
                rank: idx + 1,
                user: stats.user,
                metric_value,
                volume: stats.volume,
                realized_pnl: stats.realized_pnl,
                return_pct: stats.return_pct,
                trade_count: stats.trade_count,
                builder_fill_count: stats.builder_fill_count,
                tainted: stats.taint_result.tainted,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hl_types::Side;
    use rust_decimal_macros::dec;
    use std::collections::HashSet;

    /// Test checker using a HashSet of trade IDs
    struct TestBuilderChecker {
        builder_trade_ids: HashSet<u64>,
    }

    impl BuilderFillChecker for TestBuilderChecker {
        fn is_builder_fill(&self, fill: &UserFill, _user: &str) -> bool {
            self.builder_trade_ids.contains(&fill.trade_id)
        }
    }

    fn make_fill(
        asset: Asset,
        side: Side,
        price: Decimal,
        size: Decimal,
        fee: Decimal,
        closed_pnl: Decimal,
        trade_id: u64,
        timestamp_ms: u64,
    ) -> UserFill {
        UserFill {
            asset,
            timestamp_ms,
            price,
            size,
            side,
            fee,
            closed_pnl,
            trade_id,
            order_id: trade_id,
            crossed: true,
            direction: "Test".to_string(),
        }
    }

    #[test]
    fn test_metric_from_str() {
        assert_eq!(LeaderboardMetric::from_str("volume"), Some(LeaderboardMetric::Volume));
        assert_eq!(LeaderboardMetric::from_str("pnl"), Some(LeaderboardMetric::Pnl));
        assert_eq!(LeaderboardMetric::from_str("returnPct"), Some(LeaderboardMetric::ReturnPct));
        assert_eq!(LeaderboardMetric::from_str("return_pct"), Some(LeaderboardMetric::ReturnPct));
        assert_eq!(LeaderboardMetric::from_str("invalid"), None);
    }

    #[test]
    fn test_calculate_user_stats_volume() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(50000), dec!(0.1), dec!(5), dec!(0), 1, 1000),
            make_fill(Asset::Btc, Side::Sell, dec!(51000), dec!(0.1), dec!(5.1), dec!(100), 2, 2000),
        ];

        let checker = TestBuilderChecker {
            builder_trade_ids: [1, 2].into_iter().collect(),
        };
        let stats = calculate_user_stats("0xuser", &fills, &checker, None, None, false);

        // Volume = (50000 * 0.1) + (51000 * 0.1) = 5000 + 5100 = 10100
        assert_eq!(stats.volume, dec!(10100));
        assert_eq!(stats.trade_count, 2);
        assert_eq!(stats.builder_fill_count, 2);
        assert!(!stats.taint_result.tainted);
    }

    #[test]
    fn test_calculate_user_stats_pnl() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(50000), dec!(0.1), dec!(5), dec!(0), 1, 1000),
            make_fill(Asset::Btc, Side::Sell, dec!(51000), dec!(0.1), dec!(5.1), dec!(100), 2, 2000),
        ];

        let checker = TestBuilderChecker {
            builder_trade_ids: [1, 2].into_iter().collect(),
        };
        let stats = calculate_user_stats("0xuser", &fills, &checker, None, None, false);

        // PnL = (0 - 5) + (100 - 5.1) = -5 + 94.9 = 89.9
        assert_eq!(stats.realized_pnl, dec!(89.9));
    }

    #[test]
    fn test_calculate_user_stats_return_pct() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(50000), dec!(0.1), dec!(5), dec!(0), 1, 1000),
            make_fill(Asset::Btc, Side::Sell, dec!(51000), dec!(0.1), dec!(5.1), dec!(100), 2, 2000),
        ];

        let checker = TestBuilderChecker {
            builder_trade_ids: [1, 2].into_iter().collect(),
        };
        let stats = calculate_user_stats("0xuser", &fills, &checker, Some(dec!(1000)), None, false);

        // PnL = 89.9, capital = 1000
        // Return % = (89.9 / 1000) * 100 = 8.99%
        assert_eq!(stats.return_pct, Some(dec!(8.99)));
    }

    #[test]
    fn test_calculate_user_stats_with_taint() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(50000), dec!(0.1), dec!(5), dec!(0), 1, 1000),
            make_fill(Asset::Btc, Side::Sell, dec!(51000), dec!(0.1), dec!(5.1), dec!(100), 2, 2000),
        ];

        // Only first trade is builder fill
        let checker = TestBuilderChecker {
            builder_trade_ids: [1].into_iter().collect(),
        };
        let stats = calculate_user_stats("0xuser", &fills, &checker, None, None, false);

        assert!(stats.taint_result.tainted);
        assert_eq!(stats.taint_result.builder_fills, 1);
        assert_eq!(stats.taint_result.tainted_fills, 1);
        assert_eq!(stats.builder_fill_count, 1);
    }

    #[test]
    fn test_calculate_user_stats_coin_filter() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(50000), dec!(0.1), dec!(5), dec!(0), 1, 1000),
            make_fill(Asset::Eth, Side::Buy, dec!(3000), dec!(1), dec!(3), dec!(0), 2, 1500),
            make_fill(Asset::Btc, Side::Sell, dec!(51000), dec!(0.1), dec!(5.1), dec!(100), 3, 2000),
        ];

        let checker = TestBuilderChecker {
            builder_trade_ids: [1, 2, 3].into_iter().collect(),
        };
        let stats = calculate_user_stats("0xuser", &fills, &checker, None, Some("BTC"), false);

        // Only BTC fills: volume = 5000 + 5100 = 10100
        assert_eq!(stats.volume, dec!(10100));
        assert_eq!(stats.trade_count, 2);
    }

    #[test]
    fn test_no_builder_checker() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(50000), dec!(0.1), dec!(5), dec!(0), 1, 1000),
            make_fill(Asset::Btc, Side::Sell, dec!(51000), dec!(0.1), dec!(5.1), dec!(100), 2, 2000),
        ];

        let checker = NoBuilderChecker;
        let stats = calculate_user_stats("0xuser", &fills, &checker, None, None, false);

        // All fills are non-builder, so user should be tainted
        assert!(stats.taint_result.tainted);
        assert_eq!(stats.builder_fill_count, 0);
    }

    #[test]
    fn test_builder_only_mode_filters_fills() {
        let fills = vec![
            make_fill(Asset::Btc, Side::Buy, dec!(50000), dec!(0.1), dec!(5), dec!(0), 1, 1000),
            make_fill(Asset::Btc, Side::Sell, dec!(51000), dec!(0.1), dec!(5.1), dec!(100), 2, 2000),
        ];

        // Only first trade is builder fill
        let checker = TestBuilderChecker {
            builder_trade_ids: [1].into_iter().collect(),
        };

        // Without builder_only: counts all fills
        let stats_all = calculate_user_stats("0xuser", &fills, &checker, None, None, false);
        assert_eq!(stats_all.volume, dec!(10100));
        assert_eq!(stats_all.trade_count, 2);

        // With builder_only: only counts builder fills
        let stats_builder = calculate_user_stats("0xuser", &fills, &checker, None, None, true);
        assert_eq!(stats_builder.volume, dec!(5000)); // Only first fill: 50000 * 0.1
        assert_eq!(stats_builder.trade_count, 1);
        assert_eq!(stats_builder.builder_fill_count, 1);
    }

    #[test]
    fn test_rank_leaderboard_by_volume() {
        let stats = vec![
            UserStats {
                user: "user1".to_string(),
                volume: dec!(1000),
                realized_pnl: dec!(50),
                return_pct: None,
                trade_count: 5,
                builder_fill_count: 5,
                taint_result: TaintAnalysisResult::default(),
            },
            UserStats {
                user: "user2".to_string(),
                volume: dec!(5000),
                realized_pnl: dec!(20),
                return_pct: None,
                trade_count: 10,
                builder_fill_count: 10,
                taint_result: TaintAnalysisResult::default(),
            },
            UserStats {
                user: "user3".to_string(),
                volume: dec!(2500),
                realized_pnl: dec!(100),
                return_pct: None,
                trade_count: 8,
                builder_fill_count: 8,
                taint_result: TaintAnalysisResult::default(),
            },
        ];

        let ranked = rank_leaderboard(stats, LeaderboardMetric::Volume, false);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].user, "user2");
        assert_eq!(ranked[0].rank, 1);
        assert_eq!(ranked[1].user, "user3");
        assert_eq!(ranked[1].rank, 2);
        assert_eq!(ranked[2].user, "user1");
        assert_eq!(ranked[2].rank, 3);
    }

    #[test]
    fn test_rank_leaderboard_by_pnl() {
        let stats = vec![
            UserStats {
                user: "user1".to_string(),
                volume: dec!(1000),
                realized_pnl: dec!(50),
                return_pct: None,
                trade_count: 5,
                builder_fill_count: 5,
                taint_result: TaintAnalysisResult::default(),
            },
            UserStats {
                user: "user2".to_string(),
                volume: dec!(5000),
                realized_pnl: dec!(20),
                return_pct: None,
                trade_count: 10,
                builder_fill_count: 10,
                taint_result: TaintAnalysisResult::default(),
            },
        ];

        let ranked = rank_leaderboard(stats, LeaderboardMetric::Pnl, false);

        assert_eq!(ranked[0].user, "user1"); // Higher PnL
        assert_eq!(ranked[1].user, "user2");
    }

    #[test]
    fn test_rank_leaderboard_includes_all_users() {
        // In builder_only mode, filtering happens at calculation time,
        // so rank_leaderboard includes all users (they may have zero metrics)
        let mut tainted_result = TaintAnalysisResult::default();
        tainted_result.tainted = true;

        let stats = vec![
            UserStats {
                user: "user1".to_string(),
                volume: dec!(5000),
                realized_pnl: dec!(100),
                return_pct: None,
                trade_count: 10,
                builder_fill_count: 5,
                taint_result: tainted_result.clone(),
            },
            UserStats {
                user: "user2".to_string(),
                volume: dec!(1000),
                realized_pnl: dec!(50),
                return_pct: None,
                trade_count: 5,
                builder_fill_count: 5,
                taint_result: TaintAnalysisResult::default(),
            },
        ];

        // Even with builder_only=true, all users are included (filtering happened at calc time)
        let ranked = rank_leaderboard(stats, LeaderboardMetric::Volume, true);

        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].user, "user1"); // Higher volume
        assert_eq!(ranked[1].user, "user2");
    }

    #[test]
    fn test_rank_leaderboard_preserves_taint_status() {
        let mut tainted_result = TaintAnalysisResult::default();
        tainted_result.tainted = true;

        let stats = vec![
            UserStats {
                user: "user1".to_string(),
                volume: dec!(5000),
                realized_pnl: dec!(100),
                return_pct: None,
                trade_count: 10,
                builder_fill_count: 5,
                taint_result: tainted_result, // Tainted
            },
            UserStats {
                user: "user2".to_string(),
                volume: dec!(1000),
                realized_pnl: dec!(50),
                return_pct: None,
                trade_count: 5,
                builder_fill_count: 5,
                taint_result: TaintAnalysisResult::default(), // Clean
            },
        ];

        let ranked = rank_leaderboard(stats, LeaderboardMetric::Volume, false);

        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].user, "user1"); // Higher volume, but tainted
        assert!(ranked[0].tainted);
        assert!(!ranked[1].tainted);
    }
}
