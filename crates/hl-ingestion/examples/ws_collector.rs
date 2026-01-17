//! Example: Real-time fill collection via WebSocket.
//!
//! This example demonstrates how to use the FillCollector to capture fills
//! in real-time, bypassing the 10,000 fill limit of the historical API.
//!
//! Usage:
//!   cargo run -p hl-ingestion --example ws_collector -- <user_address>
//!
//! The collector will run for 60 seconds, printing fill counts periodically.

use hl_ingestion::{FillCollector, Network};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("hl_ingestion=debug,info")
        .init();

    // Get user address from command line
    let args: Vec<String> = env::args().collect();
    let user = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("0x1234567890abcdef1234567890abcdef12345678");

    println!("Starting fill collector for user: {}", user);
    println!("Network: Mainnet");
    println!("Duration: 60 seconds");
    println!();

    // Create collector
    let collector = FillCollector::new(Network::Mainnet);

    // Start collecting
    let handle = collector.start(user).await?;

    println!("Collector started! Waiting for fills...");
    println!();

    // Monitor for 60 seconds
    for i in 1..=12 {
        tokio::time::sleep(Duration::from_secs(5)).await;

        let count = collector.fill_count().await;
        println!(
            "[{:2}s] Fills collected: {}",
            i * 5,
            count
        );

        // Show some fill details if we have any
        if count > 0 && i % 4 == 0 {
            let fills = collector.get_fills().await;
            if let Some(last) = fills.last() {
                println!(
                    "       Latest: {} {} {} @ {} (PnL: {})",
                    last.coin,
                    if last.side == hl_ingestion::Side::Bid { "BUY" } else { "SELL" },
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

    Ok(())
}
