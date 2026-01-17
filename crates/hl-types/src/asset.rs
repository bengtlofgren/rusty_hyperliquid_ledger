//! Asset enumeration for Hyperliquid trading pairs.
//!
//! This module provides the [`Asset`] enum which represents tradeable assets
//! on Hyperliquid. Known assets have dedicated variants for type safety,
//! while unknown assets use the [`Asset::Other`] variant.
//!
//! # Hyperliquid Asset Naming
//!
//! Hyperliquid uses string identifiers for assets (e.g., "BTC", "ETH").
//! Some assets have special prefixes:
//! - `k` prefix: Indicates a 1000x multiplier (e.g., "kPEPE" = 1000 PEPE)
//! - `@` prefix: Spot tokens
//!
//! # Example
//!
//! ```rust
//! use hl_types::Asset;
//!
//! let btc = Asset::from_symbol("BTC");
//! assert_eq!(btc, Asset::Btc);
//! assert_eq!(btc.symbol(), "BTC");
//!
//! let unknown = Asset::from_symbol("NEWCOIN");
//! assert_eq!(unknown, Asset::Other("NEWCOIN".to_string()));
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Tradeable asset on Hyperliquid.
///
/// Known assets have dedicated variants for type safety and ergonomics.
/// Unknown or new assets are represented by [`Asset::Other`].
///
/// # Perpetual vs Spot
///
/// This enum primarily represents perpetual futures assets. Spot tokens
/// typically have an `@` prefix in their symbol (e.g., "@1" for spot USDC).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum Asset {
    // Major assets
    Btc,
    Eth,
    Sol,
    Bnb,
    Xrp,
    Doge,

    // DeFi
    Aave,
    Uni,
    Link,
    Mkr,
    Comp,
    Crv,
    Snx,
    Ldo,
    Gmx,

    // Layer 1s
    Avax,
    Atom,
    Dot,
    Ada,
    Trx,
    Ltc,
    Bch,
    Apt,
    Sui,
    Sei,
    Inj,
    Near,
    Ftm,
    Ton,

    // Layer 2s & Scaling
    Arb,
    Op,
    Matic,
    Stx,
    Imx,
    Zro,

    // Gaming & Metaverse
    Axs,
    Sand,
    Mana,
    Gala,
    Enj,
    Ygg,
    Bigtime,

    // Memecoins (k-prefixed are 1000x)
    KPepe,
    KShib,
    KFloki,
    KBonk,
    Wif,
    Wld,

    // Infrastructure
    Fil,
    Ar,
    Grt,
    Rune,
    Rndr,
    Tia,
    Pyth,

    // Exchange tokens
    Ftt,
    Ape,
    Blur,
    Dydx,

    // Others
    Cfx,
    Ark,
    Trb,
    Banana,
    Ordi,
    Sats,
    Hype,
    Move,

    /// Unknown or new asset not yet added to the enum.
    /// Contains the raw symbol string from the API.
    Other(String),
}

impl Asset {
    /// Create an Asset from a symbol string.
    ///
    /// Known symbols are mapped to their corresponding variants.
    /// Unknown symbols create an [`Asset::Other`] variant.
    ///
    /// # Case Sensitivity
    ///
    /// Symbol matching is case-insensitive for known assets.
    /// The [`Asset::Other`] variant preserves the original case.
    pub fn from_symbol(symbol: &str) -> Self {
        match symbol.to_uppercase().as_str() {
            // Major
            "BTC" => Asset::Btc,
            "ETH" => Asset::Eth,
            "SOL" => Asset::Sol,
            "BNB" => Asset::Bnb,
            "XRP" => Asset::Xrp,
            "DOGE" => Asset::Doge,

            // DeFi
            "AAVE" => Asset::Aave,
            "UNI" => Asset::Uni,
            "LINK" => Asset::Link,
            "MKR" => Asset::Mkr,
            "COMP" => Asset::Comp,
            "CRV" => Asset::Crv,
            "SNX" => Asset::Snx,
            "LDO" => Asset::Ldo,
            "GMX" => Asset::Gmx,

            // Layer 1s
            "AVAX" => Asset::Avax,
            "ATOM" => Asset::Atom,
            "DOT" => Asset::Dot,
            "ADA" => Asset::Ada,
            "TRX" => Asset::Trx,
            "LTC" => Asset::Ltc,
            "BCH" => Asset::Bch,
            "APT" => Asset::Apt,
            "SUI" => Asset::Sui,
            "SEI" => Asset::Sei,
            "INJ" => Asset::Inj,
            "NEAR" => Asset::Near,
            "FTM" => Asset::Ftm,
            "TON" => Asset::Ton,

            // Layer 2s
            "ARB" => Asset::Arb,
            "OP" => Asset::Op,
            "MATIC" => Asset::Matic,
            "STX" => Asset::Stx,
            "IMX" => Asset::Imx,
            "ZRO" => Asset::Zro,

            // Gaming
            "AXS" => Asset::Axs,
            "SAND" => Asset::Sand,
            "MANA" => Asset::Mana,
            "GALA" => Asset::Gala,
            "ENJ" => Asset::Enj,
            "YGG" => Asset::Ygg,
            "BIGTIME" => Asset::Bigtime,

            // Memecoins
            "KPEPE" => Asset::KPepe,
            "KSHIB" => Asset::KShib,
            "KFLOKI" => Asset::KFloki,
            "KBONK" => Asset::KBonk,
            "WIF" => Asset::Wif,
            "WLD" => Asset::Wld,

            // Infrastructure
            "FIL" => Asset::Fil,
            "AR" => Asset::Ar,
            "GRT" => Asset::Grt,
            "RUNE" => Asset::Rune,
            "RNDR" => Asset::Rndr,
            "TIA" => Asset::Tia,
            "PYTH" => Asset::Pyth,

            // Exchange
            "FTT" => Asset::Ftt,
            "APE" => Asset::Ape,
            "BLUR" => Asset::Blur,
            "DYDX" => Asset::Dydx,

            // Others
            "CFX" => Asset::Cfx,
            "ARK" => Asset::Ark,
            "TRB" => Asset::Trb,
            "BANANA" => Asset::Banana,
            "ORDI" => Asset::Ordi,
            "SATS" => Asset::Sats,
            "HYPE" => Asset::Hype,
            "MOVE" => Asset::Move,

            _ => Asset::Other(symbol.to_string()),
        }
    }

    /// Get the symbol string for this asset.
    ///
    /// Returns the canonical symbol as used by the Hyperliquid API.
    pub fn symbol(&self) -> &str {
        match self {
            // Major
            Asset::Btc => "BTC",
            Asset::Eth => "ETH",
            Asset::Sol => "SOL",
            Asset::Bnb => "BNB",
            Asset::Xrp => "XRP",
            Asset::Doge => "DOGE",

            // DeFi
            Asset::Aave => "AAVE",
            Asset::Uni => "UNI",
            Asset::Link => "LINK",
            Asset::Mkr => "MKR",
            Asset::Comp => "COMP",
            Asset::Crv => "CRV",
            Asset::Snx => "SNX",
            Asset::Ldo => "LDO",
            Asset::Gmx => "GMX",

            // Layer 1s
            Asset::Avax => "AVAX",
            Asset::Atom => "ATOM",
            Asset::Dot => "DOT",
            Asset::Ada => "ADA",
            Asset::Trx => "TRX",
            Asset::Ltc => "LTC",
            Asset::Bch => "BCH",
            Asset::Apt => "APT",
            Asset::Sui => "SUI",
            Asset::Sei => "SEI",
            Asset::Inj => "INJ",
            Asset::Near => "NEAR",
            Asset::Ftm => "FTM",
            Asset::Ton => "TON",

            // Layer 2s
            Asset::Arb => "ARB",
            Asset::Op => "OP",
            Asset::Matic => "MATIC",
            Asset::Stx => "STX",
            Asset::Imx => "IMX",
            Asset::Zro => "ZRO",

            // Gaming
            Asset::Axs => "AXS",
            Asset::Sand => "SAND",
            Asset::Mana => "MANA",
            Asset::Gala => "GALA",
            Asset::Enj => "ENJ",
            Asset::Ygg => "YGG",
            Asset::Bigtime => "BIGTIME",

            // Memecoins
            Asset::KPepe => "kPEPE",
            Asset::KShib => "kSHIB",
            Asset::KFloki => "kFLOKI",
            Asset::KBonk => "kBONK",
            Asset::Wif => "WIF",
            Asset::Wld => "WLD",

            // Infrastructure
            Asset::Fil => "FIL",
            Asset::Ar => "AR",
            Asset::Grt => "GRT",
            Asset::Rune => "RUNE",
            Asset::Rndr => "RNDR",
            Asset::Tia => "TIA",
            Asset::Pyth => "PYTH",

            // Exchange
            Asset::Ftt => "FTT",
            Asset::Ape => "APE",
            Asset::Blur => "BLUR",
            Asset::Dydx => "DYDX",

            // Others
            Asset::Cfx => "CFX",
            Asset::Ark => "ARK",
            Asset::Trb => "TRB",
            Asset::Banana => "BANANA",
            Asset::Ordi => "ORDI",
            Asset::Sats => "SATS",
            Asset::Hype => "HYPE",
            Asset::Move => "MOVE",

            Asset::Other(s) => s,
        }
    }

    /// Check if this is a known asset (not `Other`).
    pub fn is_known(&self) -> bool {
        !matches!(self, Asset::Other(_))
    }

    /// Check if this asset uses the k-prefix (1000x multiplier).
    pub fn is_kilo_asset(&self) -> bool {
        matches!(
            self,
            Asset::KPepe | Asset::KShib | Asset::KFloki | Asset::KBonk
        ) || matches!(self, Asset::Other(s) if s.starts_with('k'))
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

impl From<&str> for Asset {
    fn from(s: &str) -> Self {
        Asset::from_symbol(s)
    }
}

impl From<String> for Asset {
    fn from(s: String) -> Self {
        Asset::from_symbol(&s)
    }
}

impl From<Asset> for String {
    fn from(asset: Asset) -> Self {
        asset.symbol().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_symbol_known() {
        assert_eq!(Asset::from_symbol("BTC"), Asset::Btc);
        assert_eq!(Asset::from_symbol("ETH"), Asset::Eth);
        assert_eq!(Asset::from_symbol("kPEPE"), Asset::KPepe);
    }

    #[test]
    fn test_from_symbol_case_insensitive() {
        assert_eq!(Asset::from_symbol("btc"), Asset::Btc);
        assert_eq!(Asset::from_symbol("Btc"), Asset::Btc);
        assert_eq!(Asset::from_symbol("BTC"), Asset::Btc);
    }

    #[test]
    fn test_from_symbol_unknown() {
        let asset = Asset::from_symbol("NEWCOIN");
        assert_eq!(asset, Asset::Other("NEWCOIN".to_string()));
        assert!(!asset.is_known());
    }

    #[test]
    fn test_symbol_round_trip() {
        let assets = vec![Asset::Btc, Asset::KPepe, Asset::Other("TEST".to_string())];
        for asset in assets {
            let symbol = asset.symbol();
            let recovered = Asset::from_symbol(symbol);
            assert_eq!(asset, recovered);
        }
    }

    #[test]
    fn test_is_kilo_asset() {
        assert!(Asset::KPepe.is_kilo_asset());
        assert!(Asset::KShib.is_kilo_asset());
        assert!(!Asset::Btc.is_kilo_asset());
        assert!(Asset::Other("kTEST".to_string()).is_kilo_asset());
    }

    #[test]
    fn test_serde_round_trip() {
        let asset = Asset::Btc;
        let json = serde_json::to_string(&asset).unwrap();
        assert_eq!(json, "\"BTC\"");

        let recovered: Asset = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, Asset::Btc);
    }
}
