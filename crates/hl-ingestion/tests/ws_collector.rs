//! Integration test for real-time fill collection via WebSocket.
//!
//! This test demonstrates how to use the FillCollector to capture fills
//! in real-time, bypassing the 10,000 fill limit of the historical API.
//!
//! Run with: cargo test -p hl-ingestion --test ws_collector -- --ignored --nocapture

use hl_ingestion::{FillCollector, Network, Side};
use std::time::Duration;

// A test address - replace with an active address for meaningful results
const TEST_ADDRESS: &str = "0x1234567890abcdef1234567890abcdef12345678";

#[tokio::test]
#[ignore] // Requires network access and runs for extended duration
async fn test_websocket_fill_collector() {
    // Initialize tracing for debug output
    let _ = tracing_subscriber::fmt()
        .with_env_filter("hl_ingestion=debug,info")
        .try_init();

    println!("Starting fill collector for user: {}", TEST_ADDRESS);
    println!("Network: Mainnet");
    println!("Duration: 30 seconds");
    println!();

    // Create collector
    let collector = FillCollector::new(Network::Mainnet);

    // Start collecting
    let handle = collector.start(TEST_ADDRESS).await.expect("Failed to start collector");

    println!("Collector started! Waiting for fills...");
    println!();

    // Monitor for 30 seconds (shorter than example for test purposes)
    for i in 1..=6 {
        tokio::time::sleep(Duration::from_secs(5)).await;

        let count = collector.fill_count().await;
        println!("[{:2}s] Fills collected: {}", i * 5, count);

        // Show some fill details if we have any
        if count > 0 && i % 2 == 0 {
            let fills = collector.get_fills().await;
            if let Some(last) = fills.last() {
                println!(
                    "       Latest: {} {} {} @ {} (PnL: {})",
                    last.coin,
                    if last.side == Side::Bid { "BUY" } else { "SELL" },
                    last.sz,
                    last.px,
                    last.closed_pnl
                );
            }
        }
    }

    println!();
    println!("Stopping collector...");

    // Stop and get final results
    handle.stop().await;

    let final_fills = collector.get_fills().await;
    println!();
    println!("=== Final Results ===");
    println!("Total fills collected: {}", final_fills.len());

    if !final_fills.is_empty() {
        // Group by asset
        let mut by_asset: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for fill in &final_fills {
            *by_asset.entry(fill.coin.clone()).or_default() += 1;
        }

        println!("\nBy asset:");
        for (asset, count) in by_asset {
            println!("  {}: {} fills", asset, count);
        }

        println!("\nTime range:");
        if let (Some(first), Some(last)) = (final_fills.first(), final_fills.last()) {
            println!("  First fill: {} ms", first.time);
            println!("  Last fill:  {} ms", last.time);
        }
    }
}
