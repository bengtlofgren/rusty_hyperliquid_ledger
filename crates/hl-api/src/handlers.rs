//! Route handlers for the API endpoints.

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{Duration, TimeZone, Utc};
use std::sync::Arc;

use crate::error::ApiError;
use crate::state::AppState;
use crate::types::{
    AssetPnLResponse, HealthResponse, LeaderboardEntryResponse, LeaderboardQuery,
    LeaderboardResponse, PnLQuery, PnLResponse, TradeResponse, TradesQuery, TradesResponse,
};
use hl_builder_data::{BuilderDataClient, FillEnricher};
use hl_indexer::leaderboard::{
    calculate_leaderboard, rank_leaderboard, FillEnricherChecker, LeaderboardConfig, NoBuilderChecker,
};
use hl_types::Asset;

/// Default limit for trades query.
const DEFAULT_TRADES_LIMIT: usize = 100;

/// Maximum limit for trades query.
const MAX_TRADES_LIMIT: usize = 1000;

/// GET /health - Health check endpoint.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /v1/trades - Fetch user trades/fills.
pub async fn get_trades(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TradesQuery>,
) -> Result<Json<TradesResponse>, ApiError> {
    // Validate user address
    if query.user.is_empty() {
        return Err(ApiError::BadRequest("user address is required".to_string()));
    }

    if !query.user.starts_with("0x") {
        return Err(ApiError::BadRequest(
            "user address must start with 0x".to_string(),
        ));
    }

    // Fetch fills from indexer
    let fills = state
        .indexer
        .get_user_fills(&query.user, query.from_ms, query.to_ms)
        .await?;

    // Filter by asset if specified
    let fills = if let Some(ref asset_filter) = query.asset {
        let target_asset = Asset::from_symbol(asset_filter);
        fills
            .into_iter()
            .filter(|f| f.asset == target_asset)
            .collect()
    } else {
        fills
    };

    // Apply limit
    let limit = query
        .limit
        .unwrap_or(DEFAULT_TRADES_LIMIT)
        .min(MAX_TRADES_LIMIT);

    let total_count = fills.len();
    let has_more = total_count > limit;

    let trades: Vec<TradeResponse> = fills.into_iter().take(limit).map(Into::into).collect();

    Ok(Json(TradesResponse {
        count: trades.len(),
        trades,
        has_more,
    }))
}

/// GET /v1/pnl - Calculate PnL for a user.
pub async fn get_pnl(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PnLQuery>,
) -> Result<Json<PnLResponse>, ApiError> {
    // Validate user address
    if query.user.is_empty() {
        return Err(ApiError::BadRequest("user address is required".to_string()));
    }

    if !query.user.starts_with("0x") {
        return Err(ApiError::BadRequest(
            "user address must start with 0x".to_string(),
        ));
    }

    // Parse asset filter if provided
    let assets: Option<Vec<Asset>> = query.assets.as_ref().map(|s| {
        s.split(',')
            .map(|sym| Asset::from_symbol(sym.trim()))
            .collect()
    });

    // Get PnL from indexer
    let summary = state
        .indexer
        .get_user_pnl(
            &query.user,
            query.from_ms,
            query.to_ms,
            assets.as_deref(),
        )
        .await?;

    // Convert to response
    let by_asset: Vec<AssetPnLResponse> = summary
        .by_asset
        .values()
        .map(Into::into)
        .collect();

    Ok(Json(PnLResponse {
        user: query.user,
        realized_pnl: summary.realized_pnl,
        total_fees: summary.total_fees,
        net_pnl: summary.net_pnl,
        fill_count: summary.fill_count,
        by_asset,
        from_ms: query.from_ms,
        to_ms: query.to_ms,
    }))
}

/// GET /v1/leaderboard - Get competition leaderboard.
pub async fn get_leaderboard(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Json<LeaderboardResponse>, ApiError> {
    // Check if competition is configured
    if !state.competition_config.is_configured() {
        return Err(ApiError::BadRequest(
            "competition not configured: COMPETITION_USERS environment variable not set".to_string(),
        ));
    }

    // Parse metric
    let metric = query.parse_metric().ok_or_else(|| {
        ApiError::BadRequest(format!(
            "invalid metric '{}': must be 'volume', 'pnl', or 'returnPct'",
            query.metric
        ))
    })?;

    // Validate returnPct requires from_ms
    if matches!(metric, hl_indexer::leaderboard::LeaderboardMetric::ReturnPct) {
        if query.from_ms.is_none() {
            return Err(ApiError::BadRequest(
                "from_ms is required for returnPct metric".to_string(),
            ));
        }
        if query.max_start_capital.is_none() {
            return Err(ApiError::BadRequest(
                "maxStartCapital is required for returnPct metric".to_string(),
            ));
        }
    }

    // Determine builder_only mode
    let builder_only = query.builder_only || state.competition_config.builder_only;

    // Build leaderboard config
    let config = LeaderboardConfig {
        target_builder: state.competition_config.target_builder.clone(),
        builder_only,
        max_start_capital: query.max_start_capital,
        coin: query.coin.clone(),
        from_ms: query.from_ms,
        to_ms: query.to_ms,
        metric,
    };

    // Calculate leaderboard based on whether builder is configured
    let (stats, builder_fills_loaded) = if let Some(ref builder_addr) = state.competition_config.target_builder {
        // Fetch builder fills for the date range
        let enricher = fetch_builder_fills(builder_addr, query.from_ms, query.to_ms).await?;
        let fills_count = enricher.total_fills();
        let checker = FillEnricherChecker::new(enricher);

        tracing::info!("Loaded {} builder fills for leaderboard", fills_count);

        let stats = calculate_leaderboard(
            &state.indexer,
            &state.competition_config.competition_users,
            &config,
            &checker,
        )
        .await?;

        (stats, fills_count)
    } else {
        // No builder configured, use no-op checker
        let checker = NoBuilderChecker;

        let stats = calculate_leaderboard(
            &state.indexer,
            &state.competition_config.competition_users,
            &config,
            &checker,
        )
        .await?;

        (stats, 0)
    };

    let total_users = stats.len();

    // Rank and filter
    let ranked = rank_leaderboard(stats, metric, builder_only);
    let filtered_users = ranked.len();

    // Convert to response types
    let entries: Vec<LeaderboardEntryResponse> = ranked.into_iter().map(Into::into).collect();

    tracing::info!(
        "Leaderboard: {} total users, {} after filtering, {} builder fills",
        total_users,
        filtered_users,
        builder_fills_loaded
    );

    Ok(Json(LeaderboardResponse {
        entries,
        metric: metric.as_str().to_string(),
        from_ms: query.from_ms,
        to_ms: query.to_ms,
        coin: query.coin,
        builder_only,
        total_users,
        filtered_users,
    }))
}

/// Fetch builder fills for a date range.
///
/// Builder data is organized by date, so we fetch all dates in the range.
async fn fetch_builder_fills(
    builder_addr: &str,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
) -> Result<FillEnricher, ApiError> {
    let client = BuilderDataClient::new(builder_addr).map_err(|e| {
        ApiError::BadRequest(format!("invalid builder address '{}': {}", builder_addr, e))
    })?;

    // Determine date range
    let now = Utc::now();
    let from_date = from_ms
        .map(|ms| Utc.timestamp_millis_opt(ms).unwrap().date_naive())
        .unwrap_or_else(|| (now - Duration::days(7)).date_naive());

    let to_date = to_ms
        .map(|ms| Utc.timestamp_millis_opt(ms).unwrap().date_naive())
        .unwrap_or_else(|| now.date_naive());

    // Collect fills from all dates in range
    let mut all_fills = Vec::new();
    let mut current_date = from_date;

    while current_date <= to_date {
        match client.fetch_fills(current_date).await {
            Ok(fills) => {
                tracing::debug!(
                    "Fetched {} builder fills for date {}",
                    fills.len(),
                    current_date
                );
                all_fills.extend(fills);
            }
            Err(e) => {
                // Log but don't fail - data might not exist for all dates
                tracing::debug!(
                    "No builder fills for date {} ({}), continuing",
                    current_date,
                    e
                );
            }
        }
        current_date += Duration::days(1);
    }

    tracing::info!(
        "Total builder fills fetched: {} (from {} to {})",
        all_fills.len(),
        from_date,
        to_date
    );

    Ok(FillEnricher::new(all_fills))
}
