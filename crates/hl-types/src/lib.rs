//! hl-types: Shared data structures for Hyperliquid Trade Ledger
//!
//! This crate defines all shared types used across the workspace including:
//! - [`Asset`] - Enumeration of known trading assets with `Other` variant for extensibility
//! - [`Position`] - A user's position in a specific asset
//! - [`UserFill`] - A fill (trade execution) with timestamp
//! - [`UserPnL`] - PnL tracking with fills partitioned by asset
//!
//! # Example
//!
//! ```rust
//! use hl_types::{Asset, UserPnL, UserFill};
//! use rust_decimal_macros::dec;
//!
//! // Create a PnL tracker for a user
//! let mut pnl = UserPnL::new("0x1234...".to_string());
//!
//! // Calculate total PnL
//! let total = pnl.calculate_pnl(None);
//!
//! // Calculate PnL for specific assets
//! let btc_eth_pnl = pnl.calculate_pnl(Some(&[Asset::Btc, Asset::Eth]));
//! ```

mod asset;
mod error;
mod fill;
mod pnl;
mod position;

pub use asset::Asset;
pub use error::TypeError;
pub use fill::{Side, UserFill};
pub use pnl::{AssetPnL, PnLSummary, UserPnL};
pub use position::Position;
