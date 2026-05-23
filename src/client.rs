//! REST client core.
//!
//! [`SchwabClient`] owns the bearer credential, a shared [`reqwest::Client`],
//! and the two Schwab base URLs (trader and market-data). It exposes:
//!
//! - public namespace accessors (e.g. [`SchwabClient::accounts`],
//!   [`SchwabClient::market_data`]) into the typed endpoint builders, and
//! - two crate-private transport accessors ([`SchwabClient::trader_http`]
//!   and [`SchwabClient::market_data_http`]) that return a [`Transport`]
//!   handle scoped to one API family. Endpoint builders dispatch
//!   through `Transport`'s HTTP-verb methods.
//!
//! Endpoint modules own URL paths, request and response shapes, and any
//! optional parameters; the `Transport` handle is the only piece that
//! knows how to combine a verb, a base URL, the bearer header, and the
//! response decoder.

use reqwest::{Method, RequestBuilder};
use serde::de::DeserializeOwned;

use crate::accounts::Accounts;
use crate::constants::{MARKET_DATA_BASE_URL, TRADER_BASE_URL};
use crate::error::{Error, Result, map_response_to_error};
use crate::market_data::MarketData;
use crate::orders::{AllOrders, Orders};
use crate::secrets::{AccountHash, AuthToken};
use crate::streamer::{self, ReadHalf, WriteHalf};
use crate::transactions::Transactions;
use crate::user_preferences::UserPreferences;

#[derive(Debug, Clone)]
pub struct SchwabClient {
    client: reqwest::Client,
    trader_base_url: String,
    market_data_base_url: String,
    auth_token: AuthToken,
}

impl SchwabClient {
    /// Construct a client with Schwab's production base URLs for both the
    /// trader and market-data APIs. Override either via
    /// [`Self::with_trader_base_url`] / [`Self::with_market_data_base_url`]
    /// for staging or test fixtures.
    pub fn new(auth_token: AuthToken) -> Self {
        Self {
            client: reqwest::Client::new(),
            trader_base_url: TRADER_BASE_URL.to_string(),
            market_data_base_url: MARKET_DATA_BASE_URL.to_string(),
            auth_token,
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

    /// Accessor for `/userPreference`.
    pub fn user_preferences(&self) -> UserPreferences<'_> {
        UserPreferences::new(self)
    }

    /// Accessor for the `/accounts/{accountNumber}/transactions*` endpoint
    /// family. `account_hash` is the encrypted value from
    /// [`crate::accounts::Accounts::numbers`].
    pub fn transactions<'a, 'b>(&'a self, account_hash: &'b AccountHash) -> Transactions<'a, 'b> {
        Transactions::new(self, account_hash)
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

    /// Accessor for the market-data endpoint families (quotes, price
    /// history, market hours, movers, instruments, options chains).
    pub fn market_data(&self) -> MarketData<'_> {
        MarketData::new(self)
    }

    /// Connect to the Schwab streamer using the connection details from
    /// `/userPreference`. Returns the read and write halves of the
    /// established session; call [`WriteHalf::login`] before any other
    /// command.
    pub async fn streamer(&self) -> Result<(ReadHalf, WriteHalf)> {
        let user_preferences = self.user_preferences().get().await?;
        let streamer_info =
            user_preferences
                .streamer_info
                .into_iter()
                .next()
                .ok_or(Error::InvalidPreference {
                    field: "streamerInfo",
                    reason: "missing".to_string(),
                })?;
        streamer::connect(streamer_info).await
    }

    /// Crate-private: handle for the trader-API transport. Endpoint
    /// builders that hit `/accounts/*`, `/orders*`, `/transactions/*`,
    /// or `/userPreference` go through this.
    pub(crate) fn trader_http(&self) -> Transport<'_> {
        Transport {
            client: self,
            base_url: &self.trader_base_url,
        }
    }

    /// Crate-private: handle for the market-data transport. Endpoint
    /// builders that hit `/quotes`, `/pricehistory`, `/chains`, etc. go
    /// through this.
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
/// All HTTP verb methods return a [`RequestBuilder`] with the bearer
/// header already attached; callers chain `.query(...)`, `.json(...)`,
/// etc. as needed and then pass the builder to one of the `execute`
/// methods to send and decode.
///
/// The convenience [`Transport::get_json`] covers the no-query GET case.
pub(crate) struct Transport<'a> {
    client: &'a SchwabClient,
    base_url: &'a str,
}

impl<'a> Transport<'a> {
    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        // Auth-token reveal is scoped to header construction; the exposed
        // string does not leave this stack frame.
        self.client
            .client
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(self.client.auth_token.expose_secret())
    }

    /// Build a GET request against `{base_url}{path}` with bearer auth.
    /// `path` is appended verbatim, so callers are responsible for
    /// URL-encoding any path segments they interpolate.
    pub(crate) fn get(&self, path: &str) -> RequestBuilder {
        self.request(Method::GET, path)
    }

    /// Build a POST with bearer auth. Chain `.json(&body)` for the body.
    pub(crate) fn post(&self, path: &str) -> RequestBuilder {
        self.request(Method::POST, path)
    }

    /// Build a PUT with bearer auth.
    pub(crate) fn put(&self, path: &str) -> RequestBuilder {
        self.request(Method::PUT, path)
    }

    /// Build a DELETE with bearer auth.
    pub(crate) fn delete(&self, path: &str) -> RequestBuilder {
        self.request(Method::DELETE, path)
    }

    /// Send a prepared [`RequestBuilder`] and return the raw
    /// [`reqwest::Response`] on 2xx. Non-2xx maps to an [`Error`] via
    /// [`map_response_to_error`]. Use this when the caller needs to
    /// inspect response headers (e.g. parsing the `Location` header
    /// after a 201).
    pub(crate) async fn execute(&self, request: RequestBuilder) -> Result<reqwest::Response> {
        let response = request.send().await?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(map_response_to_error(response).await)
        }
    }

    /// Send a prepared [`RequestBuilder`] and decode the JSON body into
    /// `T` on 2xx, or map the response to an [`Error`].
    pub(crate) async fn execute_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<T> {
        let response = self.execute(request).await?;
        Ok(response.json::<T>().await?)
    }

    /// Convenience: GET + decode for endpoints that take no query
    /// parameters. Builders with query params build the request via
    /// [`Self::get`] + `.query(...)` and finish with
    /// [`Self::execute_json`].
    pub(crate) async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.execute_json(self.get(path)).await
    }
}
