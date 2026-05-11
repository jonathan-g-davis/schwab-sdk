use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, Clone)]
pub struct SchwabClient {
    client: reqwest::Client,
    base_url: String,
    auth_token: SecretString,
}

impl SchwabClient {
    pub fn new(base_url: String, auth_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            auth_token: SecretString::from(auth_token),
        }
    }

    pub async fn get_user_preferences(&self) -> reqwest::Result<Result<UserPreferences, ServiceError>> {
        let url = format!("{}/userPreference", self.base_url);
        let response = self.client.get(url).bearer_auth(self.auth_token.expose_secret()).send().await?;
        println!("Response: {:#?}", response);
        if !response.status().is_success() {
            let error = response.json::<ServiceError>().await?;
            return Ok(Err(ServiceError {
                message: error.message,
                errors: error.errors,
            }));
        }

        let body = response.json::<UserPreferences>().await?;
        Ok(Ok(body))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceError {
    pub message: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserPreferenceAccount {
    #[serde(rename = "accountNumber")]
    pub account_number: String,
    #[serde(rename = "primaryAccount")]
    pub primary_account: bool,
    #[serde(rename = "type")]
    pub account_type: String,
    #[serde(rename = "nickName")]
    pub nickname: String,
    #[serde(rename = "accountColor")]
    pub account_color: String,
    #[serde(rename = "displayAcctId")]
    pub display_account_id: String,
    #[serde(rename = "autoPositionEffect")]
    pub auto_position_effect: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamerInfo {
    #[serde(rename = "streamerSocketUrl")]
    pub streamer_socket_url: String,
    #[serde(rename = "schwabClientCustomerId")]
    pub schwab_client_customer_id: String,
    #[serde(rename = "schwabClientCorrelId")]
    pub schwab_client_correlation_id: String,
    #[serde(rename = "schwabClientChannel")]
    pub schwab_client_channel: String,
    #[serde(rename = "schwabClientFunctionId")]
    pub schwab_client_function_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Offer {
    #[serde(rename = "level2Permissions")]
    pub level2_permissions: bool,
    #[serde(rename = "mktDataPermission")]
    pub market_data_permission: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserPreferences {
    #[serde(rename = "accounts")]
    pub accounts: Vec<UserPreferenceAccount>,
    #[serde(rename = "streamerInfo")]
    pub streamer_info: Vec<StreamerInfo>,
    #[serde(rename = "offers")]
    pub offers: Vec<Offer>,
}
