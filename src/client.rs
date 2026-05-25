//! REST client core.
//!
//! [`SchwabClient`] owns a bearer-credential source ([`TokenProvider`]),
//! a shared [`reqwest::Client`], and the two Schwab base URLs (trader
//! and market-data). It exposes:
//!
//! - public namespace accessors (e.g. [`SchwabClient::accounts`],
//!   [`SchwabClient::market_data`]) into the typed endpoint builders, and
//! - two transport accessors ([`SchwabClient::trader_http`] and
//!   [`SchwabClient::market_data_http`]) that return a [`Transport`]
//!   handle scoped to one API family. Endpoint builders dispatch
//!   through `Transport`'s HTTP-verb methods.
//!
//! Endpoint modules own URL paths, request and response shapes, and any
//! optional parameters; the `Transport` handle is the only piece that
//! knows how to combine a verb, a base URL, the bearer header, and the
//! response decoder. Verb methods return an [`AuthedRequest`] that
//! defers the bearer fetch to its own `.send()` / `.send_json()`, so
//! the [`TokenProvider`] is consulted once per request, just before the
//! network write.

use std::sync::Arc;

use reqwest::{Method, RequestBuilder};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::accounts::Accounts;
use crate::constants::{MARKET_DATA_BASE_URL, TRADER_BASE_URL};
use crate::error::{Error, Result, map_response_to_error};
use crate::market_data::MarketData;
use crate::orders::{AllOrders, Orders};
use crate::secrets::{AccountHash, AuthToken};
use crate::streamer::{self, ReadHalf, WriteHalf};
use crate::token::{StaticTokenProvider, TokenProvider};
use crate::transactions::Transactions;
use crate::user_preferences::UserPreferences;

/// An HTTP client for the Charles Schwab Trader API.
///
/// Holds a [`TokenProvider`] that supplies the bearer credential for
/// every REST request. Use the namespace accessors ([`Self::accounts`],
/// [`Self::orders`], [`Self::market_data`], etc.) to construct typed
/// request builders. Use [`Self::streamer`] to open the streaming
/// WebSocket session.
///
/// The client is backed by `reqwest::Client` and is therefore cheap to
/// `Clone`; clones share the same connection pool and the same token
/// provider, so a token rotation observed through one clone is observed
/// by every clone. Reuse is encouraged over creating new instances per
/// request.
#[derive(Clone)]
pub struct SchwabClient {
    client: reqwest::Client,
    trader_base_url: String,
    market_data_base_url: String,
    token_provider: Arc<dyn TokenProvider + Send + Sync>,
}

// Manual `Debug` because `dyn TokenProvider` does not require `Debug`.
// The token provider's contents are credentials by definition; printing
// only the field's presence keeps clients safe to `dbg!`.
impl std::fmt::Debug for SchwabClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchwabClient")
            .field("trader_base_url", &self.trader_base_url)
            .field("market_data_base_url", &self.market_data_base_url)
            .field("token_provider", &"<dyn TokenProvider>")
            .finish()
    }
}

impl SchwabClient {
    /// Construct a client with Schwab's production base URLs for both the
    /// trader and market-data APIs, backed by a [`StaticTokenProvider`]
    /// wrapping `auth_token`.
    ///
    /// For a client that can pick up rotated tokens without being
    /// reconstructed, use [`Self::with_token_provider`].
    ///
    /// Override either base URL via [`Self::with_trader_base_url`] /
    /// [`Self::with_market_data_base_url`] for staging or test fixtures.
    pub fn new(auth_token: AuthToken) -> Self {
        Self::with_token_provider(Arc::new(StaticTokenProvider::new(auth_token)))
    }

    /// Construct a client backed by a caller-supplied [`TokenProvider`].
    ///
    /// The provider is consulted once per REST request. Sharing an `Arc`
    /// across `SchwabClient` clones means a token rotation observed
    /// through any clone is observed by every clone.
    pub fn with_token_provider(provider: Arc<dyn TokenProvider + Send + Sync>) -> Self {
        Self {
            client: reqwest::Client::new(),
            trader_base_url: TRADER_BASE_URL.to_string(),
            market_data_base_url: MARKET_DATA_BASE_URL.to_string(),
            token_provider: provider,
        }
    }

    /// Override the trader base URL (default: [`TRADER_BASE_URL`]).
    pub fn with_trader_base_url(mut self, url: impl Into<String>) -> Self {
        self.trader_base_url = url.into();
        self
    }

    /// Override the market-data base URL (default: [`MARKET_DATA_BASE_URL`]).
    pub fn with_market_data_base_url(mut self, url: impl Into<String>) -> Self {
        self.market_data_base_url = url.into();
        self
    }

    /// Accessor for the `/accounts*` endpoint family.
    pub fn accounts(&self) -> Accounts<'_> {
        Accounts::new(self)
    }

    /// Accessor for the `/accounts/{accountNumber}/orders*` endpoint
    /// family (single-account scope).
    pub fn orders<'a, 'b>(&'a self, account_hash: &'b AccountHash) -> Orders<'a, 'b> {
        Orders::new(self, account_hash)
    }

    /// Accessor for `/orders` - the cross-account order list. Schwab caps
    /// the date window at 60 days for this endpoint.
    pub fn orders_all(&self) -> AllOrders<'_> {
        AllOrders::new(self)
    }

    /// Accessor for the `/accounts/{accountNumber}/transactions*` endpoint
    /// family. `account_hash` is the encrypted value from
    /// [`crate::accounts::Accounts::numbers`].
    pub fn transactions<'a, 'b>(&'a self, account_hash: &'b AccountHash) -> Transactions<'a, 'b> {
        Transactions::new(self, account_hash)
    }

    /// Accessor for `/userPreference`.
    pub fn user_preferences(&self) -> UserPreferences<'_> {
        UserPreferences::new(self)
    }

    /// Accessor for the market-data endpoint families (quotes, price
    /// history, market hours, movers, instruments, options chains).
    pub fn market_data(&self) -> MarketData<'_> {
        MarketData::new(self)
    }

    /// Connect to the Schwab streamer using the connection details from
    /// `/userPreference`. Returns the read and write halves of the
    /// established session; call [`WriteHalf::login`] before any other
    /// command.
    ///
    /// `/userPreference` returns `array<UserPreference>`; this picks the
    /// first entry and the first `streamerInfo` block within it.
    /// [`streamer::connect`] validates that every field it needs is
    /// present, returning [`Error::InvalidPreference`] otherwise.
    pub async fn streamer(&self) -> Result<(ReadHalf, WriteHalf)> {
        let preferences = self
            .user_preferences()
            .get()
            .await?
            .into_iter()
            .next()
            .ok_or(Error::InvalidPreference {
                field: "userPreference",
                reason: "empty response".to_string(),
            })?;
        let streamer_info =
            preferences
                .streamer_info
                .into_iter()
                .next()
                .ok_or(Error::InvalidPreference {
                    field: "streamerInfo",
                    reason: "missing".to_string(),
                })?;
        streamer::connect(streamer_info, self.token_provider.clone()).await
    }

    /// Handle for the trader-API transport. Endpoint builders that hit
    /// `/accounts/*`, `/orders*`, `/transactions/*`, or `/userPreference`
    /// go through this.
    pub(crate) fn trader_http(&self) -> Transport<'_> {
        Transport {
            client: self,
            base_url: &self.trader_base_url,
        }
    }

    /// Handle for the market-data transport. Endpoint builders that hit
    /// `/quotes`, `/pricehistory`, `/chains`, etc. go through this.
    pub(crate) fn market_data_http(&self) -> Transport<'_> {
        Transport {
            client: self,
            base_url: &self.market_data_base_url,
        }
    }
}

/// Transport handle scoped to one Schwab API family (trader or market
/// data). Construct via [`SchwabClient::trader_http`] or
/// [`SchwabClient::market_data_http`]; the handle owns no state of its
/// own beyond borrows of the parent client.
///
/// All HTTP verb methods return an [`AuthedRequest`] borrowed from the
/// underlying [`SchwabClient`]. Callers chain `.query(...)` / `.json(...)`
/// as needed and finish with `.send()` or `.send_json()`; the bearer
/// header is attached just before the network write.
///
/// The convenience [`Transport::get_json`] covers the no-query GET case.
pub(crate) struct Transport<'a> {
    client: &'a SchwabClient,
    base_url: &'a str,
}

impl<'a> Transport<'a> {
    fn request(&self, method: Method, path: &str) -> AuthedRequest<'a> {
        AuthedRequest {
            builder: self
                .client
                .client
                .request(method, format!("{}{}", self.base_url, path)),
            provider: &*self.client.token_provider,
        }
    }

    /// Build a GET request against `{base_url}{path}`.
    pub(crate) fn get(&self, path: &str) -> AuthedRequest<'a> {
        self.request(Method::GET, path)
    }

    /// Build a POST. Chain `.json(&body)` for the body.
    pub(crate) fn post(&self, path: &str) -> AuthedRequest<'a> {
        self.request(Method::POST, path)
    }

    /// Build a PUT.
    pub(crate) fn put(&self, path: &str) -> AuthedRequest<'a> {
        self.request(Method::PUT, path)
    }

    /// Build a DELETE.
    pub(crate) fn delete(&self, path: &str) -> AuthedRequest<'a> {
        self.request(Method::DELETE, path)
    }

    /// Convenience: GET + decode for endpoints that take no query
    /// parameters. Builders with query params chain `.query(...)` onto
    /// [`Self::get`] and finish with [`AuthedRequest::send_json`].
    pub(crate) async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.get(path).send_json().await
    }
}

/// A pending request that has not yet had a bearer header attached.
///
/// Returned by [`Transport::get`] / `post` / `put` / `delete`. Wraps a
/// `reqwest::RequestBuilder` plus a borrow of the client's
/// [`TokenProvider`]. The provider is consulted exactly once per
/// request, inside [`Self::send`], so a token rotation observed between
/// the verb call and the `.send().await` is the one that goes on the
/// wire.
pub(crate) struct AuthedRequest<'a> {
    builder: RequestBuilder,
    provider: &'a (dyn TokenProvider + Send + Sync),
}

impl<'a> AuthedRequest<'a> {
    /// Add query parameters. Forwards to
    /// [`reqwest::RequestBuilder::query`]; chain multiple `.query(...)`
    /// calls to append parameters incrementally.
    pub(crate) fn query<Q: Serialize + ?Sized>(mut self, q: &Q) -> Self {
        self.builder = self.builder.query(q);
        self
    }

    /// Set the request body to the JSON serialization of `body`.
    /// Forwards to [`reqwest::RequestBuilder::json`].
    pub(crate) fn json<T: Serialize + ?Sized>(mut self, body: &T) -> Self {
        self.builder = self.builder.json(body);
        self
    }

    /// Consult the [`TokenProvider`], attach the bearer header, send
    /// the request, and return the raw [`reqwest::Response`] on 2xx.
    /// Non-2xx maps to an [`Error`] via [`map_response_to_error`].
    ///
    /// A provider failure surfaces as [`Error::TokenProvider`] without
    /// any network I/O. Use this directly when the caller needs to
    /// inspect response headers (e.g. parsing the `Location` header
    /// after a 201).
    pub(crate) async fn send(self) -> Result<reqwest::Response> {
        let token = self.provider.access_token().await?;
        // The exposed string does not leave this stack frame; it is
        // copied into the `Authorization` header by `bearer_auth`.
        let response = self
            .builder
            .bearer_auth(token.expose_secret())
            .send()
            .await?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(map_response_to_error(response).await)
        }
    }

    /// Send the request and decode the JSON body into `T` on 2xx.
    ///
    /// Body bytes are read first so a malformed response body produces
    /// [`Error::Codec`] rather than [`Error::Transport`]; transport
    /// errors are reserved for network-level failures (DNS, connect,
    /// TLS, I/O).
    pub(crate) async fn send_json<T: DeserializeOwned>(self) -> Result<T> {
        let response = self.send().await?;
        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Codec {
            context: "decode response body".to_string(),
            reason: e.to_string(),
        })
    }
}
