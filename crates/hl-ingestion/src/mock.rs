//! Mock data source for testing.
//!
//! `MockSource` implements `DataSource` with configurable responses,
//! allowing tests to run without network calls.
//!
//! # Usage
//!
//! ```rust,ignore
//! use hl_ingestion::{DataSource, MockSource};
//!
//! let mock = MockSource::new()
//!     .with_fills(vec![/* test fills */]);
//!
//! let fills = mock.get_user_fills("0x...", None, None).await?;
//! ```

use crate::{error::IngestionError, DataSource};
use hypersdk::hypercore::types::{ClearinghouseState, Fill, UserBalance};

/// Mock data source for testing.
///
/// Stores test data that will be returned by `DataSource` methods.
/// Uses builder pattern for convenient setup.
///
/// # Note on Cloning
///
/// This mock clones data when returning it. For testing purposes
/// this overhead is negligible. The real `HyperliquidSource` returns
/// owned data from network responses, so there's no additional cost there.
#[derive(Default, Clone)]
pub struct MockSource {
    /// Fills to return from `get_user_fills`.
    pub fills: Vec<Fill>,

    /// Clearinghouse state to return. If None, returns an error.
    pub clearinghouse_state: Option<ClearinghouseState>,

    /// User balances to return from `get_user_balances`.
    pub user_balances: Vec<UserBalance>,
}

impl MockSource {
    /// Create a new empty mock source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the fills to return (builder pattern).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mock = MockSource::new().with_fills(vec![fill1, fill2]);
    /// ```
    pub fn with_fills(mut self, fills: Vec<Fill>) -> Self {
        self.fills = fills;
        self
    }

    /// Set the clearinghouse state to return (builder pattern).
    pub fn with_clearinghouse_state(mut self, state: ClearinghouseState) -> Self {
        self.clearinghouse_state = Some(state);
        self
    }

    /// Set the user balances to return (builder pattern).
    pub fn with_user_balances(mut self, balances: Vec<UserBalance>) -> Self {
        self.user_balances = balances;
        self
    }
}

impl DataSource for MockSource {
    async fn get_user_fills(
        &self,
        _user: &str,
        from_ms: Option<i64>,
        to_ms: Option<i64>,
    ) -> Result<Vec<Fill>, IngestionError> {
        // Filter fills by time window, just like the real implementation.
        // This ensures tests behave consistently with production code.
        let fills = self
            .fills
            .iter()
            .filter(|f| {
                let t = f.time as i64;
                let after_from = from_ms.map_or(true, |from| t >= from);
                let before_to = to_ms.map_or(true, |to| t <= to);
                after_from && before_to
            })
            .cloned()
            .collect();

        Ok(fills)
    }

    async fn get_clearinghouse_state(
        &self,
        _user: &str,
    ) -> Result<ClearinghouseState, IngestionError> {
        self.clearinghouse_state
            .clone()
            .ok_or_else(|| IngestionError::NoData("mock clearinghouse state not configured".into()))
    }

    async fn get_user_balances(&self, _user: &str) -> Result<Vec<UserBalance>, IngestionError> {
        Ok(self.user_balances.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_mock() {
        let mock = MockSource::new();
        let fills = mock.get_user_fills("0x123", None, None).await.unwrap();
        assert!(fills.is_empty());
    }

    #[tokio::test]
    async fn test_clearinghouse_not_configured() {
        let mock = MockSource::new();
        let result = mock.get_clearinghouse_state("0x123").await;
        assert!(result.is_err());
    }
}
