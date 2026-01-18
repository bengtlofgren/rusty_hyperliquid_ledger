//! Integration test for builder enrichment for specific wallets.
//!
//! Run with: cargo test -p hl-indexer --test builder_test --features builder-enrichment -- --ignored --nocapture

#![cfg(feature = "builder-enrichment")]

use hl_indexer::{Indexer, IndexerConfig};

const WALLETS: &[(&str, &str)] = &[
    ("Wallet 1", "0x0e09b56ef137f417e424f1265425e93bfff77e17"),
    ("Wallet 2", "0x186b7610ff3f2e3fd7985b95f525ee0e37a74"),
    ("Wallet 3", "0x6c8031a9eb4415284f3f89c0420f697c87168263"),
];

const BUILDER_ADDRESS: &str = "0x2868fc0d9786a740b491577a43502259efa78a39";

// Jan 1, 2026 00:00:00 UTC in milliseconds
const FROM_MS: i64 = 1767225600000;

#[tokio::test]
#[ignore] // Requires network access and builder-enrichment feature
async fn test_builder_fill_attribution() {
    println!("=== Builder Fill Attribution Test ===\n");
    println!("Builder: {}", BUILDER_ADDRESS);
    println!("Time range: Jan 1, 2026 - now\n");

    // Create indexer with builder enrichment
    let config = IndexerConfig::mainnet().with_builder(BUILDER_ADDRESS);
    let indexer = Indexer::new(config);

    println!("Builder enrichment enabled: {}\n", indexer.has_builder_enrichment());

    for (name, wallet) in WALLETS {
        println!("--- {} ({}) ---", name, wallet);

        match indexer
            .get_user_fills_with_builder_info(wallet, Some(FROM_MS), None)
            .await
        {
            Ok(result) => {
                println!("  Total fills: {}", result.fills.len());
                println!("  Builder fills matched: {}", result.builder_fills_matched);
                println!("  Total builder fees: {}", result.total_builder_fees);

                let pct = if result.fills.is_empty() {
                    0.0
                } else {
                    (result.builder_fills_matched as f64 / result.fills.len() as f64) * 100.0
                };
                println!("  Builder fill %: {:.2}%", pct);
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }
        println!();
    }

    println!("=== Done ===");
}
