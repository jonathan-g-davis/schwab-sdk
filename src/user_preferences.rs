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
#[derive(Debug)]
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

/// `GET /userPreference` response body.
//
// Schwab's spec types this endpoint as `array<UserPreference>`, but the live
// API returns a single object.
#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct UserPreference {
    /// Per-account preferences (nickname, default position effect, etc.).
    #[serde(rename = "accounts", default)]
    pub accounts: Vec<UserPreferenceAccount>,
    /// Streamer connection blocks. Typically one element; the first is what
    /// [`SchwabClient::streamer`](crate::SchwabClient::streamer) uses.
    #[serde(rename = "streamerInfo", default)]
    pub streamer_info: Vec<StreamerInfo>,
    /// Market-data entitlements (level 1, level 2, etc.).
    #[serde(rename = "offers", default)]
    pub offers: Vec<Offer>,
}

/// Per-account entry inside a [`UserPreference`].
#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct UserPreferenceAccount {
    /// Plain account number.
    #[serde(rename = "accountNumber")]
    pub account_number: Option<AccountNumber>,
    /// `true` if this is the client's primary account.
    #[serde(rename = "primaryAccount", default)]
    pub primary_account: bool,
    /// Account type as Schwab labels it (`"MARGIN"`, `"CASH"`, etc.).
    #[serde(rename = "type")]
    pub account_type: Option<String>,
    /// Client-chosen nickname for the account.
    #[serde(rename = "nickName")]
    pub nickname: Option<String>,
    /// Schwab UI color tag (`"Green"` or `"Blue"`).
    #[serde(rename = "accountColor")]
    pub account_color: Option<String>,
    /// Masked id Schwab displays (e.g. `"...5678"`).
    #[serde(rename = "displayAcctId")]
    pub display_account_id: Option<AccountNumber>,
    /// `true` if Schwab should auto-determine `position_effect`
    /// (open / close) on submitted orders.
    #[serde(rename = "autoPositionEffect", default)]
    pub auto_position_effect: bool,
}

/// Streamer connection details. Every property is optional per the spec;
/// `SchwabClient::streamer` (and [`crate::streamer::connect`]) validate
/// that the fields it actually needs are present, returning
/// [`crate::Error::InvalidPreference`] if any required value is missing.
#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct StreamerInfo {
    /// WebSocket URL to connect to (`wss://...`).
    #[serde(rename = "streamerSocketUrl")]
    pub streamer_socket_url: Option<String>,
    /// `schwabClientCustomerId` echoed back into every streamer request envelope.
    #[serde(rename = "schwabClientCustomerId")]
    pub schwab_client_customer_id: Option<CustomerId>,
    /// Per-session correlation id, attached to every frame for support tracing.
    #[serde(rename = "schwabClientCorrelId")]
    pub schwab_client_correlation_id: Option<String>,
    /// Schwab channel string (e.g. `"N9"`).
    #[serde(rename = "schwabClientChannel")]
    pub schwab_client_channel: Option<String>,
    /// Schwab function id (e.g. `"APIAPP"`).
    #[serde(rename = "schwabClientFunctionId")]
    pub schwab_client_function_id: Option<String>,
}

/// Market-data entitlement entry.
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Offer {
    /// `true` if the account is entitled to level-2 (order-book) data.
    #[serde(rename = "level2Permissions", default)]
    pub level2_permissions: bool,
    /// Market-data permission code Schwab assigned (e.g. `"NP"` for
    /// non-professional).
    #[serde(rename = "mktDataPermission")]
    pub market_data_permission: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_canonical_payload() {
        // Single object with every documented field populated.
        // Schwab API spec types this as an array, but it really only sends one object.
        let body = r#"{
            "accounts": [
                {
                    "accountNumber": "12345678",
                    "primaryAccount": true,
                    "type": "MARGIN",
                    "nickName": "main",
                    "accountColor": "Green",
                    "displayAcctId": "...5678",
                    "autoPositionEffect": false
                }
            ],
            "streamerInfo": [
                {
                    "streamerSocketUrl": "wss://streamer-api.schwab.com/ws",
                    "schwabClientCustomerId": "CUSTID",
                    "schwabClientCorrelId": "abc-123",
                    "schwabClientChannel": "N9",
                    "schwabClientFunctionId": "APIAPP"
                }
            ],
            "offers": [
                {
                    "level2Permissions": true,
                    "mktDataPermission": "NP"
                }
            ]
        }"#;

        let p: UserPreference = serde_json::from_str(body).unwrap();
        assert_eq!(p.accounts.len(), 1);
        assert!(p.accounts[0].primary_account);
        assert_eq!(p.accounts[0].nickname.as_deref(), Some("main"));
        assert_eq!(p.streamer_info.len(), 1);
        assert_eq!(
            p.streamer_info[0].streamer_socket_url.as_deref(),
            Some("wss://streamer-api.schwab.com/ws"),
        );
        assert_eq!(p.offers.len(), 1);
        assert!(p.offers[0].level2_permissions);
        assert_eq!(p.offers[0].market_data_permission.as_deref(), Some("NP"));
    }

    #[test]
    fn deserializes_minimal_payload() {
        // No required fields per the spec; empty objects, missing arrays, and
        // missing booleans must still decode.
        let body = r#"{
            "accounts": [{}],
            "streamerInfo": [{}],
            "offers": [{}]
        }"#;

        let p: UserPreference = serde_json::from_str(body).unwrap();
        assert_eq!(p.accounts.len(), 1);
        assert!(p.accounts[0].account_number.is_none());
        assert!(!p.accounts[0].primary_account);
        assert!(!p.accounts[0].auto_position_effect);
        assert!(p.accounts[0].nickname.is_none());
        assert_eq!(p.streamer_info.len(), 1);
        assert!(p.streamer_info[0].streamer_socket_url.is_none());
        assert!(p.streamer_info[0].schwab_client_customer_id.is_none());
        assert_eq!(p.offers.len(), 1);
        assert!(!p.offers[0].level2_permissions);
        assert!(p.offers[0].market_data_permission.is_none());
    }

    #[test]
    fn deserializes_when_top_level_arrays_missing() {
        // Spec lists no required fields on UserPreference; a response with no
        // `accounts`/`streamerInfo`/`offers` keys must decode to empty vecs.
        let p: UserPreference = serde_json::from_str("{}").unwrap();
        assert!(p.accounts.is_empty());
        assert!(p.streamer_info.is_empty());
        assert!(p.offers.is_empty());
    }
}
