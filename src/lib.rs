//! Typed Rust client for the Charles Schwab Trader API and streamer WebSocket.
//!
//! All features of the Trader API are exposed through [`SchwabClient`]. From
//! it, namespace accessors return typed endpoint builders:
//!
//! - [`SchwabClient::accounts`] - `/accounts*`
//! - [`SchwabClient::orders`] / [`SchwabClient::orders_all`] - `/orders*`
//! - [`SchwabClient::transactions`] - `/accounts/{accountNumber}/transactions*`
//! - [`SchwabClient::user_preferences`] - `/userPreference`
//! - [`SchwabClient::market_data`] - quotes, price history, market hours,
//!   movers, instruments, option chains, expiration chains
//! - [`SchwabClient::streamer`] - opens the streamer WebSocket and returns
//!   a [`streamer::ReadHalf`] / [`streamer::WriteHalf`] pair
//!
//! All money and quantity fields use [`rust_decimal::Decimal`]; secrets
//! ([`AuthToken`], [`CustomerId`], [`AccountNumber`], [`AccountHash`])
//! are wrapped in newtypes that redact in `Debug`.
//!
//! What this crate does **not** include:
//!
//! - The OAuth authorization-code flow. Callers obtain a bearer token
//!   themselves and hand it to [`SchwabClient::new`].
//! - Retry and rate limiting. Each [`Error`] exposes
//!   [`Error::is_retryable`] and [`Error::retry_after`] so a caller can
//!   layer a policy (`backon`, etc.) on top.
//! - Order placement at scale. Place / replace / cancel / preview exist,
//!   but the Schwab API exposes no client-controllable idempotency key;
//!   callers that need retry-safe submission must dedupe at their own
//!   layer.
//!
//! # Examples
//!
//! Construct a client from a bearer token and make a call. The token is
//! obtained out of band; this crate does not perform the OAuth
//! authorization-code exchange.
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient};
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let token = AuthToken::new(std::env::var("SCHWAB_ACCESS_TOKEN").unwrap());
//! let client = SchwabClient::new(token);
//!
//! // Namespace accessors are methods on the client.
//! let accounts = client.accounts().numbers().await?;
//! println!("{} linked account(s)", accounts.len());
//! # Ok(())
//! # }
//! ```
//!
//! End to end: resolve an account, read a quote, and act on it.
//!
//! ```no_run
//! use rust_decimal_macros::dec;
//! use schwab_sdk::{AuthToken, SchwabClient};
//! use schwab_sdk::market_data::QuoteEntry;
//! use schwab_sdk::orders::OrderRequest;
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! // 1. Resolve the encrypted account hash. Every per-account endpoint
//! //    takes this hash in its `{accountNumber}` path segment, never the
//! //    plain account number.
//! let accounts = client.accounts().numbers().await?;
//! let account = accounts.first().expect("at least one linked account");
//! let account_hash = &account.hash_value;
//!
//! // 2. Read a quote and pull the last price out of the typed entry. An
//! //    unknown symbol comes back as `QuoteEntry::Error`, not an `Err`.
//! let quotes = client.market_data().quotes().list(["AAPL"]).send().await?;
//! let last_price = match quotes.get("AAPL") {
//!     Some(QuoteEntry::Equity(q)) => q.quote.as_ref().and_then(|inner| inner.last_price),
//!     _ => None,
//! };
//! let Some(last_price) = last_price else {
//!     println!("no quote for AAPL");
//!     return Ok(());
//! };
//!
//! // 3. Place a limit buy just under the last trade; keep the order id
//! //    Schwab returns so the fill can be polled later.
//! let limit = last_price - dec!(0.50);
//! let order_id = client
//!     .orders(account_hash)
//!     .place(OrderRequest::buy_limit("AAPL", dec!(10), limit))
//!     .await?;
//! println!("placed order {order_id} at {limit}");
//! # Ok(())
//! # }
//! ```

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
mod token;

pub(crate) mod macros;

pub mod accounts;
pub mod error;
pub mod market_data;
pub mod orders;
pub mod secrets;
pub mod streamer;
pub mod transactions;
pub mod user_preferences;

// Re-exports of dependencies whose types appear in the public API.
pub use chrono;
pub use http;
pub use rust_decimal;

pub use client::SchwabClient;
pub use constants::{
    DEFAULT_AUTH_TOKEN_EXPIRY, DEFAULT_REFRESH_TOKEN_EXPIRY, MARKET_DATA_BASE_URL, TRADER_BASE_URL,
};
pub use error::{Error, ErrorBody, Result};
pub use secrets::{AccountHash, AccountNumber, AuthToken, CustomerId};
pub use streamer::{StreamerResponse, WebSocketError};
pub use token::{StaticTokenProvider, TokenProvider};
