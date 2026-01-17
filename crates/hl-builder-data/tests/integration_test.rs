//! Integration tests for hl-builder-data.
//!
//! These tests download real data from the Hyperliquid stats endpoint.

use chrono::NaiveDate;
use hl_builder_data::{BuilderDataClient, BuilderDataError, FillEnricher};

/// The insilico builder address (lowercase as required).
const BUILDER_ADDRESS: &str = "0x2868fc0d9786a740b491577a43502259efa78a39";

#[tokio::test]
async fn test_fetch_fills_single_date() {
    let client = BuilderDataClient::new(BUILDER_ADDRESS).unwrap();

    // Fetch fills for January 10, 2026
    let date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let fills = client.fetch_fills(date).await.unwrap();

    // Should have some fills
    assert!(!fills.is_empty(), "Expected fills for {}", date);
    println!("Fetched {} fills for {}", fills.len(), date);

    // Check first fill structure
    let first = &fills[0];
    assert!(!first.user.is_empty());
    assert!(!first.asset.symbol().is_empty());
    println!(
        "First fill: {} {} {} @ {} (builder_fee: {})",
        first.user,
        first.asset,
        first.size,
        first.price,
        first.builder_fee
    );
}

#[tokio::test]
async fn test_fetch_fills_date_range() {
    let client = BuilderDataClient::new(BUILDER_ADDRESS).unwrap();

    // Fetch fills for January 10-12, 2026
    let from = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 1, 12).unwrap();

    let fills = client.fetch_fills_range(from, to).await.unwrap();

    println!(
        "Fetched {} fills for range {} to {}",
        fills.len(),
        from,
        to
    );

    // Should have fills (at least from Jan 10)
    assert!(!fills.is_empty());

    // Fills should be sorted by time
    for i in 1..fills.len() {
        assert!(
            fills[i].time >= fills[i - 1].time,
            "Fills should be sorted by time"
        );
    }
}

#[tokio::test]
async fn test_fetch_fills_not_found() {
    let client = BuilderDataClient::new(BUILDER_ADDRESS).unwrap();

    // Try a date that likely doesn't have data (far future)
    let date = NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
    let result = client.fetch_fills(date).await;

    assert!(
        matches!(result, Err(BuilderDataError::NotFound { .. })),
        "Expected NotFound error for future date"
    );
}

#[tokio::test]
async fn test_builder_address_lowercase() {
    // Should accept uppercase and convert to lowercase
    let client = BuilderDataClient::new("0x2868FC0D9786A740B491577A43502259EFA78A39").unwrap();
    assert_eq!(client.builder_address(), BUILDER_ADDRESS);
}

#[tokio::test]
async fn test_enricher_with_real_data() {
    let client = BuilderDataClient::new(BUILDER_ADDRESS).unwrap();

    // Fetch fills
    let date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let fills = client.fetch_fills(date).await.unwrap();

    // Create enricher
    let enricher = FillEnricher::new(fills);

    // Check totals
    let total_fees = enricher.total_builder_fees();
    let total_volume = enricher.total_volume();

    println!(
        "Total fills: {}, Total fees: {}, Total volume: {}",
        enricher.total_fills(),
        total_fees,
        total_volume
    );

    assert!(enricher.total_fills() > 0);
    assert!(total_fees > rust_decimal::Decimal::ZERO);
    assert!(total_volume > rust_decimal::Decimal::ZERO);
}

#[tokio::test]
async fn test_csv_structure_validation() {
    let client = BuilderDataClient::new(BUILDER_ADDRESS).unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
    let fills = client.fetch_fills(date).await.unwrap();

    // Validate each fill has expected structure
    for fill in &fills {
        // User should be a valid address
        assert!(fill.user.starts_with("0x"), "User should be an address");
        assert!(fill.user.len() == 42, "User address should be 42 chars");

        // Counterparty should also be valid
        assert!(
            fill.counterparty.starts_with("0x"),
            "Counterparty should be an address"
        );

        // Prices and sizes should be positive
        assert!(fill.price > rust_decimal::Decimal::ZERO);
        assert!(fill.size > rust_decimal::Decimal::ZERO);

        // Builder fee should be non-negative
        assert!(fill.builder_fee >= rust_decimal::Decimal::ZERO);
    }
}

#[tokio::test]
async fn test_multiple_dates_2026() {
    let client = BuilderDataClient::new(BUILDER_ADDRESS).unwrap();

    // Test several dates in January 2026
    let dates = [
        NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
    ];

    for date in dates {
        match client.fetch_fills(date).await {
            Ok(fills) => {
                println!("{}: {} fills", date, fills.len());
            }
            Err(BuilderDataError::NotFound { .. }) => {
                println!("{}: no data", date);
            }
            Err(e) => {
                panic!("Unexpected error for {}: {}", date, e);
            }
        }
    }
}
