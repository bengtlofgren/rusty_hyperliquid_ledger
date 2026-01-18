//! Integration test to fetch real data from Hyperliquid and verify responses.
//!
//! Run with: cargo test -p hl-ingestion --test fetch_data -- --ignored --nocapture

use hl_ingestion::{DataSource, HyperliquidSource};

// A known active address on Hyperliquid for testing
// HLP vault address - should have lots of activity
const TEST_ADDRESS: &str = "0x010461c14e146ac35fe42271bdc1134ee31c703a";

#[tokio::test]
#[ignore] // Requires network access to Hyperliquid API
async fn test_fetch_clearinghouse_state() {
    println!("Fetching clearinghouse state...");
    let source = HyperliquidSource::mainnet();

    match source.get_clearinghouse_state(TEST_ADDRESS).await {
        Ok(state) => {
            println!("   Success!");
            println!("   Margin summary: {:?}", state.margin_summary);
            println!("   Withdrawable: {}", state.withdrawable);
            println!("   Positions count: {}", state.asset_positions.len());
            for pos in &state.asset_positions {
                println!("     - {:?}", pos);
            }
        }
        Err(e) => {
            panic!("Failed to fetch clearinghouse state: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires network access to Hyperliquid API
async fn test_fetch_user_balances() {
    println!("Fetching user balances...");
    let source = HyperliquidSource::mainnet();

    match source.get_user_balances(TEST_ADDRESS).await {
        Ok(balances) => {
            println!("   Success! Got {} balances:", balances.len());
            for bal in &balances {
                println!("     - {:?}", bal);
            }
        }
        Err(e) => {
            panic!("Failed to fetch user balances: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires network access to Hyperliquid API
async fn test_fetch_fills_no_time_filter() {
    println!("Fetching fills (no time filter)...");
    let source = HyperliquidSource::mainnet();

    match source.get_user_fills(TEST_ADDRESS, None, None).await {
        Ok(fills) => {
            println!("   Success! Got {} fills.", fills.len());
            if !fills.is_empty() {
                println!("   First 5 fills:");
                for fill in fills.iter().take(5) {
                    println!(
                        "     - {} {} {} @ {} (pnl: {}, fee: {}, time: {})",
                        fill.side, fill.sz, fill.coin, fill.px, fill.closed_pnl, fill.fee, fill.time
                    );
                }

                // Show time range
                if let (Some(first), Some(last)) = (fills.first(), fills.last()) {
                    println!("   Time range: {} to {}", first.time, last.time);

                    // Convert to human readable
                    let first_dt = chrono::DateTime::from_timestamp_millis(first.time as i64);
                    let last_dt = chrono::DateTime::from_timestamp_millis(last.time as i64);
                    if let (Some(f), Some(l)) = (first_dt, last_dt) {
                        println!("   Date range: {} to {}", f, l);
                    }
                }
            }
        }
        Err(e) => {
            panic!("Failed to fetch fills: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires network access to Hyperliquid API
async fn test_fetch_fills_last_24_hours() {
    println!("Fetching fills (last 24 hours) with pagination...");
    let source = HyperliquidSource::mainnet();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let day_ago_ms = now_ms - (24 * 60 * 60 * 1000);

    match source
        .get_user_fills(TEST_ADDRESS, Some(day_ago_ms), Some(now_ms))
        .await
    {
        Ok(fills) => {
            println!(
                "   Got {} fills in last 24 hours (uses userFillsByTime)",
                fills.len()
            );
        }
        Err(e) => {
            panic!("Failed to fetch fills for last 24 hours: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires network access to Hyperliquid API
async fn test_fetch_fills_last_7_days_pagination() {
    println!("Fetching fills (last 7 days) with pagination...");
    let source = HyperliquidSource::mainnet();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let week_ago_ms = now_ms - (7 * 24 * 60 * 60 * 1000);

    match source
        .get_user_fills(TEST_ADDRESS, Some(week_ago_ms), Some(now_ms))
        .await
    {
        Ok(fills) => {
            println!("   Got {} fills in last 7 days", fills.len());
            if fills.len() >= 2000 {
                println!("   (Pagination worked! Got more than 2000 fills)");
            }
            if !fills.is_empty() {
                // Show time range
                let first_time = fills.iter().map(|f| f.time).max().unwrap();
                let last_time = fills.iter().map(|f| f.time).min().unwrap();
                let first_dt = chrono::DateTime::from_timestamp_millis(first_time as i64);
                let last_dt = chrono::DateTime::from_timestamp_millis(last_time as i64);
                if let (Some(f), Some(l)) = (first_dt, last_dt) {
                    println!("   Date range: {} to {}", l, f);
                }
            }
        }
        Err(e) => {
            panic!("Failed to fetch fills for last 7 days: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires network access to Hyperliquid API
async fn test_invalid_address_handling() {
    println!("Testing invalid address handling...");
    let source = HyperliquidSource::mainnet();

    match source.get_user_fills("not-a-valid-address", None, None).await {
        Ok(_) => panic!("Expected error for invalid address but got success"),
        Err(e) => println!("   Got expected error: {}", e),
    }
}
