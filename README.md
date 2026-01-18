# Hyperliquid Trade Ledger

A high-performance trade ledger and PnL tracking system for Hyperliquid, built with Rust.

## Features

- **Trade Fetching**: Fetch user trades/fills from Hyperliquid API with pagination support
- **PnL Calculation**: Calculate realized PnL, fees, and net PnL with per-asset breakdown
- **Real-Time Collection**: WebSocket-based fill collection to bypass the 10k fill limit
- **Builder Attribution**: Optional enrichment with builder fill data
- **REST API**: Clean HTTP API for integration with other services
- **Multi-Network**: Support for both Mainnet and Testnet

## Architecture

```
hl-server (binary)
├── hl-api         HTTP endpoints (Axum)
│   └── hl-indexer     Business logic & data transformation
│       ├── hl-ingestion   Data fetching (hypersdk + WebSocket)
│       ├── hl-builder-data Builder attribution (optional)
│       └── hl-types       Shared data structures
```

## Quick Start

### Prerequisites

- Rust 1.75+ (for native async traits)
- Docker (optional)

### Running Locally

```bash
# Clone the repository
git clone <repo-url>
cd hyperliquid_ledger

# Build and run
cargo run -p hl-server

# Or with environment variables
NETWORK=mainnet PORT=3000 cargo run -p hl-server
```

### Running with Docker

```bash
# Build the image
docker build -t hl-server .

# Run the container
docker run -p 3000:3000 -e NETWORK=mainnet hl-server

# Or use docker-compose
docker-compose up
```

## Configuration

Environment variables (can be set in a `.env` file):

| Variable | Description | Default |
|----------|-------------|---------|
| `NETWORK` | Network to connect to (`mainnet` or `testnet`) | `mainnet` |
| `HOST` | Server bind address | `0.0.0.0` |
| `PORT` | Server port | `3000` |
| `FILL_SOURCE` | Fill source (`api` or `websocket`) | `api` |
| `RUST_LOG` | Log level filter | `info` |
| `TARGET_BUILDER` | Builder address for taint detection (lowercase) | - |
| `BUILDER_ONLY` | Filter leaderboard to builder-only users (`true`/`false`) | `false` |
| `COMPETITION_USERS` | Comma-separated list of competition participant addresses | - |

## API Endpoints

### Health Check

```bash
GET /health
```

Response:
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

### Fetch User Trades

```bash
GET /v1/trades?user=0x...&from_ms=...&to_ms=...&asset=...&limit=...
```

Query Parameters:
| Parameter | Required | Description |
|-----------|----------|-------------|
| `user` | Yes | User wallet address (0x...) |
| `from_ms` | No | Start time (ms since epoch) |
| `to_ms` | No | End time (ms since epoch) |
| `asset` | No | Filter by asset symbol (e.g., "BTC") |
| `limit` | No | Max results (default: 100, max: 1000) |

Response:
```json
{
  "trades": [
    {
      "asset": "ETH",
      "timestamp_ms": 1768576560527,
      "price": "3310.0",
      "size": "4.1724",
      "side": "buy",
      "fee": "2.071596",
      "closed_pnl": "0.0",
      "trade_id": 461128571856302,
      "order_id": 295755350723,
      "crossed": false,
      "direction": "Open Long"
    }
  ],
  "count": 1,
  "has_more": false
}
```

### Calculate PnL

```bash
GET /v1/pnl?user=0x...&from_ms=...&to_ms=...&assets=BTC,ETH
```

Query Parameters:
| Parameter | Required | Description |
|-----------|----------|-------------|
| `user` | Yes | User wallet address (0x...) |
| `from_ms` | No | Start time (ms since epoch) |
| `to_ms` | No | End time (ms since epoch) |
| `assets` | No | Comma-separated asset filter |

Response:
```json
{
  "user": "0x...",
  "realized_pnl": "11308.860868",
  "total_fees": "307.39658378",
  "net_pnl": "11001.46428422",
  "fill_count": 1023,
  "by_asset": [
    {
      "asset": "ETH",
      "realized_pnl": "312.708280",
      "fees": "210.937421",
      "net_pnl": "101.770859",
      "fill_count": 39,
      "volume": "602678.39180"
    }
  ],
  "from_ms": null,
  "to_ms": null
}
```

### Get Competition Leaderboard

```bash
GET /v1/leaderboard?metric=volume&fromMs=...&toMs=...&coin=...&builderOnly=...&maxStartCapital=...
```

Query Parameters:
| Parameter | Required | Description |
|-----------|----------|-------------|
| `metric` | No | Ranking metric: `volume`, `pnl`, or `returnPct` (default: `volume`) |
| `fromMs` | No* | Start time (ms since epoch). *Required for `returnPct` |
| `toMs` | No | End time (ms since epoch) |
| `coin` | No | Filter by asset symbol (e.g., "BTC") |
| `builderOnly` | No | Only show non-tainted users (`true`/`false`) |
| `maxStartCapital` | No* | Capital for return % calculation. *Required for `returnPct` |

Response:
```json
{
  "entries": [
    {
      "rank": 1,
      "user": "0xabc...",
      "metricValue": "1523456.78",
      "volume": "1523456.78",
      "realizedPnl": "12345.67",
      "returnPct": "12.35",
      "tradeCount": 156,
      "tainted": false
    }
  ],
  "metric": "volume",
  "fromMs": 1768694400000,
  "toMs": null,
  "coin": null,
  "builderOnly": false,
  "totalUsers": 10,
  "filteredUsers": 8
}
```

**Taint Detection**: A user is "tainted" if any fill during an open position did not go through the target builder. When `builderOnly=true`, tainted users are excluded from the leaderboard.

## Using as a Library

You can also use the crates directly in your Rust project:

```rust
use hl_indexer::{Indexer, IndexerConfig, FillSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create indexer (API mode - default)
    let indexer = Indexer::mainnet();

    // Fetch trades
    let fills = indexer.get_user_fills(
        "0x1234567890abcdef1234567890abcdef12345678",
        Some(1704067200000), // from_ms
        None,                // to_ms
    ).await?;

    println!("Got {} fills", fills.len());

    // Calculate PnL
    let pnl = indexer.get_user_pnl(
        "0x1234567890abcdef1234567890abcdef12345678",
        None, None, None,
    ).await?;

    println!("Net PnL: {}", pnl.net_pnl);
    Ok(())
}
```

### WebSocket Mode (Unlimited Fills)

For competitions or high-volume traders, use WebSocket mode to bypass the 10k fill limit:

```rust
use hl_indexer::{Indexer, IndexerConfig, FillSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create indexer in WebSocket mode
    let config = IndexerConfig::mainnet()
        .with_fill_source(FillSource::WebSocket);
    let indexer = Indexer::new(config);

    // Start collecting BEFORE the competition
    indexer.start_collecting("0x...").await?;

    // ... competition runs ...

    // Get ALL collected fills (no 10k limit!)
    let fills = indexer.get_user_fills("0x...", None, None).await?;
    println!("Collected {} fills", fills.len());

    // Stop when done
    indexer.stop_collecting().await;
    Ok(())
}
```

### Builder Attribution

Enable the `builder-enrichment` feature for builder fill attribution:

```toml
[dependencies]
hl-indexer = { path = "crates/hl-indexer", features = ["builder-enrichment"] }
```

```rust
let config = IndexerConfig::mainnet()
    .with_builder("0x2868fc0d9786a740b491577a43502259efa78a39");

let indexer = Indexer::new(config);

let result = indexer.get_user_fills_with_builder_info(
    "0x...",
    Some(from_ms),
    Some(to_ms),
).await?;

println!("Builder fills matched: {}", result.builder_fills_matched);
println!("Total builder fees: {}", result.total_builder_fees);
```

## Project Structure

```
hyperliquid_ledger/
├── crates/
│   ├── hl-types/        # Shared data types (Asset, Position, UserFill, PnL)
│   ├── hl-ingestion/    # Data fetching layer (HTTP API + WebSocket)
│   ├── hl-builder-data/ # Builder attribution data (optional)
│   ├── hl-indexer/      # Business logic and data transformation
│   ├── hl-api/          # HTTP API layer (Axum)
│   └── hl-server/       # Main binary
├── Cargo.toml           # Workspace configuration
├── Dockerfile           # Docker build
└── docker-compose.yml   # Docker Compose setup
```

## Known Limitations

### Historical Fill Limit (10,000 fills)

The Hyperliquid public API (`userFillsByTime` endpoint) limits historical fill retrieval to a maximum of **10,000 fills per user**, even with pagination.

**Workaround**: Use WebSocket mode (`FillSource::WebSocket`) to capture fills in real-time with no limit. Start the collector before your event begins.

Currently these trades are stored in memory, and for longer running competitions, one might want to incorporate some data-server separation. Perhaps dumping collected data to a database and running queries on that.

### Builder Attribution Delay

Builder fill data is uploaded daily with ~24h delay. The builder address must be **entirely lowercase**, and requests return 403 if no fills exist for that builder on that date. This means that builder attribution can only be used for days prior to the current day.

### Competition Users Must Be Known Beforehand

Competition participants must be configured via the `COMPETITION_USERS` environment variable at server startup. There is currently no API endpoint to dynamically add or remove users during runtime.

**Workaround**: Restart the server with an updated `COMPETITION_USERS` list, or modify the codebase to add a POST endpoint with `RwLock`-based mutable state (see Architecture Considerations below).

## Development

### Running Tests

```bash
# Run all unit tests
cargo test --workspace

# Run with builder-enrichment feature
cargo test --workspace --features builder-enrichment

# Run a single test
cargo test -p hl-indexer test_name
```

### Integration Tests

Integration tests require network access to Hyperliquid APIs and are ignored by default:

```bash
# Data fetching tests (clearinghouse state, balances, fills, pagination)
cargo test -p hl-ingestion --test fetch_data -- --ignored --nocapture

# WebSocket fill collector test
cargo test -p hl-ingestion --test ws_collector -- --ignored --nocapture

# Builder attribution test (requires builder-enrichment feature)
cargo test -p hl-indexer --test builder_test --features builder-enrichment -- --ignored --nocapture
```

### Building

```bash
# Debug build
cargo build --workspace

# Release build
cargo build --workspace --release
```

## Architecture Considerations

### Adding Dynamic User Management

The current architecture uses immutable state (`Arc<AppState>`) for thread-safe sharing across Axum handlers. To add dynamic user registration via POST:

**Required Changes:**

1. **Wrap mutable state in `RwLock`:**
   ```rust
   pub struct AppState {
       pub indexer: Indexer,
       pub competition_config: RwLock<CompetitionConfig>,
   }
   ```

2. **Add POST endpoint in `handlers.rs`:**
   ```rust
   pub async fn add_competition_user(
       State(state): State<Arc<AppState>>,
       Json(payload): Json<AddUserRequest>,
   ) -> Result<Json<()>, ApiError> {
       let mut config = state.competition_config.write().await;
       config.competition_users.push(payload.user.to_lowercase());
       Ok(Json(()))
   }
   ```

3. **Update existing handlers** to use `.read().await` when accessing `competition_config`.

4. **Consider authentication** to prevent unauthorized modifications.

## License

MIT
