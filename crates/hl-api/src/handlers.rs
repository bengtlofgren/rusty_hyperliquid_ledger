//! Route handlers for the API endpoints.

use axum::{
    extract::{Query, State},
    Json,
};
use std::sync::Arc;

use crate::error::ApiError;
use crate::state::AppState;
use crate::types::{
    AssetPnLResponse, HealthResponse, PnLQuery, PnLResponse, TradeResponse, TradesQuery,
    TradesResponse,
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
