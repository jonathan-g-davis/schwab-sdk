//! REST client core.
//!
//! [`SchwabClient`] is a thin reqwest wrapper that owns the bearer
//! credential and the base URL. Endpoint logic lives in [`crate::api`];
//! this module knows about HTTP, JSON, and Schwab's error response shape,
//! and nothing else.

use http::Uri;

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

    pub async fn get_user_preferences(&self) -> Result<UserPreferences> {
        let url = format!("{}/userPreference", self.base_url);
        // `auth_token` reveal is scoped to header construction; do not store
        // or pass the raw string elsewhere.
        let response = self
            .client
            .get(url)
            .bearer_auth(self.auth_token.expose_secret())
            .send()
            .await?;
        if response.status().is_success() {
            Ok(response.json::<UserPreferences>().await?)
        } else {
            Err(map_response_to_error(response).await)
        }
    }

    pub async fn streamer(&self) -> Result<SchwabStreamer> {
        let user_preferences = self.get_user_preferences().await?;
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
}
