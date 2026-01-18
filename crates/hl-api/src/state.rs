//! Application state for the API server.

use hl_indexer::Indexer;

/// Configuration for trading competitions.
#[derive(Debug, Clone)]
pub struct CompetitionConfig {
    /// Target builder address (must be lowercase).
    pub target_builder: Option<String>,

    /// Whether to enforce builder-only mode by default.
    pub builder_only: bool,

    /// List of user addresses participating in the competition.
    pub competition_users: Vec<String>,
}

impl Default for CompetitionConfig {
    fn default() -> Self {
        Self {
            target_builder: None,
            builder_only: false,
            competition_users: Vec::new(),
        }
    }
}

impl CompetitionConfig {
    /// Create a new CompetitionConfig from environment variables.
    ///
    /// Environment variables:
    /// - `TARGET_BUILDER`: Builder address (will be lowercased)
    /// - `BUILDER_ONLY`: "true" to enable builder-only mode
    /// - `COMPETITION_USERS`: Comma-separated list of user addresses
    pub fn from_env() -> Self {
        let target_builder = std::env::var("TARGET_BUILDER")
            .ok()
            .map(|s| s.to_lowercase());

        let builder_only = std::env::var("BUILDER_ONLY")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(false);

        let competition_users = std::env::var("COMPETITION_USERS")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|addr| addr.trim().to_lowercase())
                    .filter(|addr| !addr.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            target_builder,
            builder_only,
            competition_users,
        }
    }

    /// Check if a competition is configured.
    pub fn is_configured(&self) -> bool {
        !self.competition_users.is_empty()
    }

    /// Check if builder-only mode is enabled.
    pub fn is_builder_only(&self) -> bool {
        self.builder_only && self.target_builder.is_some()
    }

    /// Get the number of competition users.
    pub fn user_count(&self) -> usize {
        self.competition_users.len()
    }
}

/// Shared application state.
pub struct AppState {
    /// The indexer for fetching and processing data.
    pub indexer: Indexer,

    /// Competition configuration.
    pub competition_config: CompetitionConfig,
}

impl AppState {
    /// Create a new application state with the given indexer.
    pub fn new(indexer: Indexer) -> Self {
        Self {
            indexer,
            competition_config: CompetitionConfig::default(),
        }
    }

    /// Create a new application state with indexer and competition config.
    pub fn with_config(indexer: Indexer, competition_config: CompetitionConfig) -> Self {
        Self {
            indexer,
            competition_config,
        }
    }
}
