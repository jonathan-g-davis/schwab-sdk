//! `schwab-sdk` - a typed Rust client for the Charles Schwab Trader API.

// Panic-family lints are denied in production code. If a future change
// genuinely needs one of these in non-test code, add `#[allow(...)]` with
// a one-line comment explaining why.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
    )
)]
#![warn(missing_docs)]

mod client;
mod constants;

pub(crate) mod macros;

pub mod accounts;
pub mod error;
pub mod market_data;
pub mod orders;
pub mod secrets;
pub mod streamer;
pub mod transactions;
pub mod user_preferences;

pub use client::SchwabClient;
pub use constants::{
    DEFAULT_AUTH_TOKEN_EXPIRY, DEFAULT_REFRESH_TOKEN_EXPIRY, MARKET_DATA_BASE_URL, TRADER_BASE_URL,
};
pub use error::{Error, ErrorBody, Result};
pub use secrets::{AccountHash, AccountNumber, AuthToken, CustomerId};
pub use streamer::{StreamerResponse, WebSocketError};
