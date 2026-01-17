# Hyperliquid ledger

A trade ledger and PnL tracking system for Hyperliquid.

## Installation instructions

WIP

## Known Limitations

### Historical Fill Limit (10,000 fills)

The Hyperliquid public API (`userFillsByTime` endpoint) limits historical fill retrieval to a maximum of **10,000 fills per user**, even with pagination. This means:

- Active traders with more than 10,000 historical trades cannot retrieve their complete trade history via the public API
- Only the most recent 10,000 fills within any requested time window are available
- For complete historical data beyond this limit, an RPC/node solution would be required

### Builder Attribution

The public API's `Fill` struct does not include builder attribution directly. However, builder fills are available separately via Hyperliquid's stats data endpoint:

```text
https://stats-data.hyperliquid.xyz/Mainnet/builder_fills/{builder_address}/{YYYYMMDD}.csv.lz4
```

The `hl-builder-data` crate provides functionality to download and parse these files for data enrichment.

- Files are uploaded daily with ~24h delay
- Builder address must be **entirely lowercase**
- Returns 403 if no fills exist for that builder on that date
