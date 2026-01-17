//! HTTP client for downloading builder fill data.

use crate::error::BuilderDataError;
use crate::parser::parse_builder_fills;
use crate::types::BuilderFill;
use chrono::NaiveDate;
use std::io::Read;

/// Base URL for Hyperliquid stats data.
const STATS_BASE_URL: &str = "https://stats-data.hyperliquid.xyz";

/// Client for fetching builder fill data from Hyperliquid.
///
/// Builder fills are uploaded daily in LZ4-compressed CSV format.
/// Files are typically available with ~24 hour delay.
///
/// # Example
///
/// ```rust,no_run
/// use hl_builder_data::BuilderDataClient;
/// use chrono::NaiveDate;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = BuilderDataClient::new(
///         "0x2868fc0d9786a740b491577a43502259efa78a39"
///     )?;
///
///     let fills = client.fetch_fills(
///         NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()
///     ).await?;
///
///     println!("Got {} fills", fills.len());
///     Ok(())
/// }
/// ```
pub struct BuilderDataClient {
    http_client: reqwest::Client,
    builder_address: String,
}

impl BuilderDataClient {
    /// Create a new client for the specified builder address.
    ///
    /// The builder address will be normalized to lowercase.
    ///
    /// # Errors
    ///
    /// Returns an error if the address doesn't start with "0x".
    pub fn new(builder_address: &str) -> Result<Self, BuilderDataError> {
        let address = builder_address.to_lowercase();

        if !address.starts_with("0x") {
            return Err(BuilderDataError::InvalidAddress(
                "address must start with 0x".to_string(),
            ));
        }

        Ok(Self {
            http_client: reqwest::Client::new(),
            builder_address: address,
        })
    }

    /// Get the builder address (lowercase).
    pub fn builder_address(&self) -> &str {
        &self.builder_address
    }

    /// Build the URL for a specific date.
    fn build_url(&self, date: NaiveDate) -> String {
        let date_str = date.format("%Y%m%d").to_string();
        format!(
            "{}/Mainnet/builder_fills/{}/{}.csv.lz4",
            STATS_BASE_URL, self.builder_address, date_str
        )
    }

    /// Fetch fills for a specific date.
    ///
    /// # Arguments
    ///
    /// * `date` - The date to fetch fills for (YYYYMMDD format internally)
    ///
    /// # Errors
    ///
    /// - `NotFound` if no data exists for that date (403/404)
    /// - `Http` for network errors
    /// - `Decompression` if LZ4 decompression fails
    /// - `CsvParse` if CSV parsing fails
    pub async fn fetch_fills(&self, date: NaiveDate) -> Result<Vec<BuilderFill>, BuilderDataError> {
        let url = self.build_url(date);
        tracing::debug!("Fetching builder fills from: {}", url);

        let response = self.http_client.get(&url).send().await?;

        // Check for 403/404 (no data)
        if response.status() == reqwest::StatusCode::FORBIDDEN
            || response.status() == reqwest::StatusCode::NOT_FOUND
        {
            return Err(BuilderDataError::NotFound {
                date: date.format("%Y-%m-%d").to_string(),
            });
        }

        // Check for other errors
        let response = response.error_for_status()?;

        // Get compressed bytes
        let compressed = response.bytes().await?;
        tracing::debug!("Downloaded {} bytes (compressed)", compressed.len());

        // Decompress LZ4
        let decompressed = decompress_lz4(&compressed)?;
        tracing::debug!("Decompressed to {} bytes", decompressed.len());

        // Parse CSV
        let fills = parse_builder_fills(&decompressed)?;
        tracing::info!(
            "Parsed {} builder fills for {}",
            fills.len(),
            date.format("%Y-%m-%d")
        );

        Ok(fills)
    }

    /// Fetch fills for a date range (inclusive).
    ///
    /// Fetches data for each date in the range. Dates with no data
    /// are skipped (not treated as errors).
    ///
    /// # Arguments
    ///
    /// * `from` - Start date (inclusive)
    /// * `to` - End date (inclusive)
    ///
    /// # Returns
    ///
    /// All fills from the date range, combined and sorted by time.
    pub async fn fetch_fills_range(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<BuilderFill>, BuilderDataError> {
        let mut all_fills = Vec::new();
        let mut current = from;

        while current <= to {
            match self.fetch_fills(current).await {
                Ok(fills) => {
                    all_fills.extend(fills);
                }
                Err(BuilderDataError::NotFound { .. }) => {
                    // Skip dates with no data
                    tracing::debug!("No data for {}", current.format("%Y-%m-%d"));
                }
                Err(e) => return Err(e),
            }
            current = current
                .succ_opt()
                .ok_or_else(|| BuilderDataError::InvalidDate("date overflow".to_string()))?;
        }

        // Sort by time
        all_fills.sort_by_key(|f| f.time);

        Ok(all_fills)
    }
}

/// Decompress LZ4 data.
fn decompress_lz4(compressed: &[u8]) -> Result<Vec<u8>, BuilderDataError> {
    let mut decoder = lz4_flex::frame::FrameDecoder::new(compressed);
    let mut decompressed = Vec::new();

    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| BuilderDataError::Decompression(e.to_string()))?;

    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client() {
        let client = BuilderDataClient::new("0xABCD1234").unwrap();
        assert_eq!(client.builder_address(), "0xabcd1234");
    }

    #[test]
    fn test_new_client_invalid() {
        let result = BuilderDataClient::new("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_url() {
        let client =
            BuilderDataClient::new("0x2868fc0d9786a740b491577a43502259efa78a39").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
        let url = client.build_url(date);

        assert_eq!(
            url,
            "https://stats-data.hyperliquid.xyz/Mainnet/builder_fills/0x2868fc0d9786a740b491577a43502259efa78a39/20260110.csv.lz4"
        );
    }
}
