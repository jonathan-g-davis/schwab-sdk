//! `schwab-sdk` - a typed Rust client for the Charles Schwab Trader API.

pub mod api;
pub mod error;
pub mod model;
pub mod rest;
pub mod streamer;
pub mod websocket;

pub use error::{Error, Result};
pub use model::{AccountHash, AccountNumber, AuthToken, CustomerId};
pub use rest::SchwabClient;
pub use streamer::SchwabStreamer;
