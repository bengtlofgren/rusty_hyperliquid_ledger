//! hl-api: HTTP API layer for trade ledger service.
//!
//! This crate provides the REST API endpoints for the Hyperliquid trade ledger:
//!
//! - `GET /health` - Health check
//! - `GET /v1/trades` - Fetch user trades/fills
//! - `GET /v1/pnl` - Calculate PnL for a user
//!
//! # Example
//!
//! ```rust,no_run
//! use hl_api::{create_router, AppState};
//! use hl_indexer::{Indexer, IndexerConfig};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let indexer = Indexer::new(IndexerConfig::mainnet());
//!     let state = Arc::new(AppState::new(indexer));
//!     let app = create_router(state);
//!
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```

mod error;
mod handlers;
mod state;
mod types;

pub use error::ApiError;
pub use state::AppState;
pub use types::*;

use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

/// Create the API router with all endpoints.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(handlers::health))
        // V1 API routes
        .route("/v1/trades", get(handlers::get_trades))
        .route("/v1/pnl", get(handlers::get_pnl))
        // Add state and middleware
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
