//! `schwab-sdk` - a typed Rust client for the Charles Schwab Trader API.

pub mod api;
pub mod client;
pub mod error;
pub mod secrets;
pub mod streamer;
pub mod websocket;

pub use client::SchwabClient;
pub use error::{Error, Result};
pub use secrets::{
    AccountHash, AccountNumber, AuthToken, CustomerId, DEFAULT_AUTH_TOKEN_EXPIRY,
    DEFAULT_REFRESH_TOKEN_EXPIRY,
};
pub use streamer::SchwabStreamer;
