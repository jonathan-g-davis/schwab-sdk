//! `GET /userPreference` - Schwab Trader API.
//!
//! Returns the caller's accounts, streamer connection info, and market-data
//! permissions. The `streamerInfo` block is what is used to construct the
//! streamer halves at connection time
//! (see [`SchwabClient::streamer`](crate::SchwabClient::streamer)).
//!
//! Reached through
//! [`SchwabClient::user_preferences`](crate::SchwabClient::user_preferences).

use crate::client::SchwabClient;
use crate::error::Result;
use crate::secrets::{AccountNumber, CustomerId};

/// Accessor for `/userPreference`. Construct via
/// [`SchwabClient::user_preferences`].
pub struct UserPreferences<'a> {
    client: &'a SchwabClient,
}

impl<'a> UserPreferences<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// `GET /userPreference` - returns the caller's preferences.
    pub async fn get(&self) -> Result<UserPreference> {
        self.client.trader_http().get_json("/userPreference").await
    }
}

/// `GET /userPreference` response body. Schwab's OpenAPI schema names this
/// `UserPreference` (singular), even though most of its fields are arrays.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UserPreference {
    #[serde(rename = "accounts")]
    pub accounts: Vec<UserPreferenceAccount>,
    #[serde(rename = "streamerInfo")]
    pub streamer_info: Vec<StreamerInfo>,
    #[serde(rename = "offers")]
    pub offers: Vec<Offer>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UserPreferenceAccount {
    #[serde(rename = "accountNumber")]
    pub account_number: AccountNumber,
    #[serde(rename = "primaryAccount")]
    pub primary_account: bool,
    #[serde(rename = "type")]
    pub account_type: String,
    #[serde(rename = "nickName")]
    pub nickname: String,
    #[serde(rename = "accountColor")]
    pub account_color: String,
    #[serde(rename = "displayAcctId")]
    pub display_account_id: AccountNumber,
    #[serde(rename = "autoPositionEffect")]
    pub auto_position_effect: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamerInfo {
    #[serde(rename = "streamerSocketUrl")]
    pub streamer_socket_url: String,
    #[serde(rename = "schwabClientCustomerId")]
    pub schwab_client_customer_id: CustomerId,
    #[serde(rename = "schwabClientCorrelId")]
    pub schwab_client_correlation_id: String,
    #[serde(rename = "schwabClientChannel")]
    pub schwab_client_channel: String,
    #[serde(rename = "schwabClientFunctionId")]
    pub schwab_client_function_id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Offer {
    #[serde(rename = "level2Permissions")]
    pub level2_permissions: bool,
    #[serde(rename = "mktDataPermission")]
    pub market_data_permission: String,
}
