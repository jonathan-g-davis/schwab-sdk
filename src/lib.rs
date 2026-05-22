//! `schwab-sdk` - a typed Rust client for the Charles Schwab Trader API.

mod client;
mod websocket;

pub(crate) mod macros;

pub mod accounts;
pub mod error;
pub mod market_data;
pub mod orders;
pub mod secrets;
pub mod streamer;
pub mod transactions;
pub mod user_preferences;

pub use client::{MARKET_DATA_BASE_URL, SchwabClient, TRADER_BASE_URL};
pub use error::{Error, ErrorBody, Result};
pub use secrets::{
    AccountHash, AccountNumber, AuthToken, CustomerId, DEFAULT_AUTH_TOKEN_EXPIRY,
    DEFAULT_REFRESH_TOKEN_EXPIRY,
};
pub use streamer::{SchwabStreamer, StreamerResponse};
pub use websocket::WebSocketError;
