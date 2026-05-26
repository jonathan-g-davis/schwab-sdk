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
//! Bearer credentials reach the SDK through the [`TokenProvider`] trait.
//! [`SchwabClient::new`] wraps a static [`AuthToken`] in a
//! [`StaticTokenProvider`]. Long-lived clients implement [`TokenProvider`]
//! over their own refresh strategy and pass it to
//! [`SchwabClient::with_token_provider`]. The provider is consulted once
//! per REST request and once per streamer LOGIN frame, so a rotated
//! token is observed on the next call without rebuilding the client.
//!
//! What this crate does **not** include:
//!
//! - The OAuth authorization-code flow. Callers obtain a bearer token
//!   out of band and hand it to [`SchwabClient::new`], or implement
//!   [`TokenProvider`] for refresh-on-demand (see its doctest for a
//!   worked provider).
//! - Retry and rate limiting. Each [`Error`] exposes
//!   [`Error::is_retryable`] and [`Error::retry_after`] so a caller can
//!   layer a policy (`backon`, etc.) on top. See the doctest on
//!   [`Error::is_retryable`] for a minimal backoff loop.
//! - Order placement at scale. Place / replace / cancel / preview exist,
//!   but the Schwab API exposes no client-controllable idempotency key;
//!   callers that need retry-safe submission must dedupe at their own
//!   layer.
//!
//! # Security
//!
//! `schwab-sdk` is built to reduce the risk of credential or PII
//! leakage through this crate, not to be a security boundary for the
//! application as a whole.
//!
//! - The secret newtypes ([`AuthToken`], [`CustomerId`],
//!   [`AccountNumber`], [`AccountHash`]) redact in `Debug` and zeroise
//!   on `Drop`. The [`secrets`] module documents what these properties
//!   cover and what they do not.
//! - The crate emits no log lines, writes no files, and does not embed
//!   secret values in [`Error`] variants. A bearer credential is
//!   materialised only at the `Authorization` header and the streamer
//!   LOGIN frame.
//! - Transport defaults to HTTPS for REST and WSS for the streamer.
//!   Release builds reject `http://` base-URL overrides and `ws://`
//!   streamer URLs; debug builds permit them so local fixture servers
//!   work in tests.
//! - Credential storage, the OAuth flow, retry policy, rate limiting,
//!   structured logging, and host-level hardening (disabling core
//!   dumps, encrypted swap) are caller responsibilities. See
//!   `SECURITY.md` in the repository for the vulnerability-reporting
//!   channel and the formal scope.
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
//!
//! # Disclaimer
//!
//! This crate is an independent client. It is **not affiliated with,
//! endorsed by, or sponsored by Charles Schwab & Co., Inc.** "Schwab"
//! and related marks are the property of their respective owners.
//!
//! The crate is provided "as is" without warranty of any kind. The
//! authors and contributors are **not responsible for any financial
//! loss, missed trades, incorrect or duplicate orders, or other trading
//! outcomes** arising from use of this crate. You are solely responsible
//! for the orders your code submits and for verifying its behavior
//! before trading real money. See the MIT and Apache-2.0 license texts
//! for the full warranty disclaimer.

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
