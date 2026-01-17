//! Application state for the API server.

use hl_indexer::Indexer;

/// Shared application state.
pub struct AppState {
    /// The indexer for fetching and processing data.
    pub indexer: Indexer,
}

impl AppState {
    /// Create a new application state with the given indexer.
    pub fn new(indexer: Indexer) -> Self {
        Self { indexer }
    }
}
