//! `schwab-rs` - a typed Rust client for the Charles Schwab Trader API.

pub mod api;
pub mod error;
pub mod model;
pub mod rest;
pub mod streamer;
pub mod token_provider;
pub mod websocket;

pub use token_provider::TokenProvider;

pub use error::{Error, Result};
pub use model::{AccountNumber, AuthToken, CustomerId};
pub use rest::SchwabClient;
pub use streamer::SchwabStreamer;
