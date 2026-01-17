//! hl-server: Main binary for Hyperliquid Trade Ledger service.
//!
//! This binary wires together all crates and starts the HTTP server.

use hl_api::{create_router, AppState};
use hl_indexer::{FillSource, Indexer, IndexerConfig, Network};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Default port for the server.
const DEFAULT_PORT: u16 = 3000;

/// Default host for the server.
const DEFAULT_HOST: &str = "0.0.0.0";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file (if present)
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hl_server=info,hl_api=info,hl_indexer=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse configuration from environment
    let network = Network::from_env();
    let host = std::env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // Parse fill source from environment (default: api)
    let fill_source = match std::env::var("FILL_SOURCE")
        .unwrap_or_else(|_| "api".to_string())
        .to_lowercase()
        .as_str()
    {
        "websocket" | "ws" => FillSource::WebSocket,
        _ => FillSource::Api,
    };

    tracing::info!(
        "Starting hl-server on {}:{} (network: {:?}, fill_source: {:?})",
        host,
        port,
        network,
        fill_source
    );

    // Create indexer with configured fill source
    let config = match network {
        Network::Mainnet => IndexerConfig::mainnet(),
        Network::Testnet => IndexerConfig::testnet(),
    }
    .with_fill_source(fill_source);

    let indexer = Indexer::new(config);

    // Create app state
    let state = Arc::new(AppState::new(indexer));

    // Create router
    let app = create_router(state);

    // Start server
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Server listening on http://{}", addr);
    tracing::info!("Endpoints:");
    tracing::info!("  GET /health       - Health check");
    tracing::info!("  GET /v1/trades    - Fetch user trades");
    tracing::info!("  GET /v1/pnl       - Calculate user PnL");

    axum::serve(listener, app).await?;

    Ok(())
}
