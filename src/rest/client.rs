//! REST client core.
//!
//! [`SchwabClient`] owns the bearer credential and the base URL and exposes
//! namespace accessors (e.g. [`SchwabClient::accounts`]) into the typed
//! endpoint builders in [`crate::api`]. The endpoint modules themselves
//! own the URL paths, request shapes, response shapes, and any optional
//! parameters; this module only provides the transport primitive
//! ([`SchwabClient::get_json`]) the builders dispatch through.

use http::Uri;
use serde::de::DeserializeOwned;

use crate::api::accounts::Accounts;
use crate::api::user_preferences::UserPreferences;
use crate::error::{Error, Result, map_response_to_error};
use crate::model::AuthToken;
use crate::{SchwabStreamer, websocket};

#[derive(Debug, Clone)]
pub struct SchwabClient {
    client: reqwest::Client,
    base_url: String,
    auth_token: AuthToken,
}

impl SchwabClient {
    pub fn new(base_url: String, auth_token: AuthToken) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            auth_token,
        }
    }

    /// Accessor for the `/accounts*` endpoint family.
    pub fn accounts(&self) -> Accounts<'_> {
        Accounts::new(self)
    }

    /// Accessor for `/userPreference`.
    pub fn user_preferences(&self) -> UserPreferences<'_> {
        UserPreferences::new(self)
    }

    /// Connect to the Schwab streamer using the connection details from
    /// `/userPreference`. Returns a ready-to-login [`SchwabStreamer`].
    pub async fn streamer(&self) -> Result<SchwabStreamer> {
        let user_preferences = self.user_preferences().get().await?;
        let streamer_info = user_preferences
            .streamer_info
            .into_iter()
            .next()
            .ok_or(Error::MissingPreference("streamerInfo"))?;
        let uri = streamer_info
            .streamer_socket_url
            .parse::<Uri>()
            .map_err(|e| Error::InvalidUri(format!("streamerSocketUrl: {e}")))?;
        let websocket = websocket::connect(uri).await?;
        SchwabStreamer::builder()
            .websocket(websocket)
            .customer_id(streamer_info.schwab_client_customer_id)
            .correlation_id(streamer_info.schwab_client_correlation_id)
            .channel(streamer_info.schwab_client_channel)
            .function_id(streamer_info.schwab_client_function_id)
            .build()
            .map_err(|e| Error::Build(e.to_string()))
    }

    /// Crate-private transport: GET `{base_url}{path}` with bearer auth,
    /// decode the JSON body into `T` on 2xx, or map the response to an
    /// [`Error`] via [`map_response_to_error`].
    ///
    /// `path` is appended verbatim, so callers are responsible for any
    /// query-string formatting and for URL-encoding path segments.
    pub(crate) async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        // Auth-token reveal is scoped to header construction; the exposed
        // string does not leave this stack frame.
        let response = self
            .client
            .get(url)
            .bearer_auth(self.auth_token.expose_secret())
            .send()
            .await?;
        if response.status().is_success() {
            Ok(response.json::<T>().await?)
        } else {
            Err(map_response_to_error(response).await)
        }
    }
}
