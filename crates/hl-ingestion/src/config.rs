//! Configuration for the ingestion layer.
//!
//! Minimal configuration - hypersdk handles RPC URLs internally,
//! so we only need to specify mainnet vs testnet.
//!
//! # Environment Variables
//!
//! - `HL_NETWORK`: "mainnet" or "testnet" (default: mainnet)

use std::env;

/// Network selection for Hyperliquid.
///
/// This is `Copy` for zero-cost passing - no heap allocation needed.
/// Default is `Mainnet` for safety (testnet should be explicitly chosen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Network {
    /// Hyperliquid mainnet (production).
    #[default]
    Mainnet,

    /// Hyperliquid testnet (for development/testing).
    Testnet,
}

impl Network {
    /// Load network selection from `HL_NETWORK` environment variable.
    ///
    /// Returns `Mainnet` if:
    /// - Variable is not set
    /// - Variable is set but not "testnet"
    ///
    /// This fail-safe behavior ensures we don't accidentally connect
    /// to testnet in production.
    ///
    /// # Example
    ///
    /// ```rust
    /// use hl_ingestion::Network;
    ///
    /// // With HL_NETWORK=testnet
    /// // let network = Network::from_env(); // Returns Testnet
    ///
    /// // With HL_NETWORK unset or any other value
    /// // let network = Network::from_env(); // Returns Mainnet
    /// ```
    pub fn from_env() -> Self {
        match env::var("HL_NETWORK")
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Ok("testnet") => Network::Testnet,
            _ => Network::Mainnet,
        }
    }

    /// Returns true if this is the testnet.
    #[inline]
    pub fn is_testnet(&self) -> bool {
        matches!(self, Network::Testnet)
    }

    /// Returns true if this is mainnet.
    #[inline]
    pub fn is_mainnet(&self) -> bool {
        matches!(self, Network::Mainnet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_mainnet() {
        assert_eq!(Network::default(), Network::Mainnet);
    }

    #[test]
    fn test_is_testnet() {
        assert!(Network::Testnet.is_testnet());
        assert!(!Network::Mainnet.is_testnet());
    }

    #[test]
    fn test_is_mainnet() {
        assert!(Network::Mainnet.is_mainnet());
        assert!(!Network::Testnet.is_mainnet());
    }
}
