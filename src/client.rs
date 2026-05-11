use http::StatusCode;
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

    pub async fn get_user_preferences(&self) -> Result<UserPreferences> {
        let url = format!("{}/userPreference", self.base_url);
        let response = self.client.get(url).bearer_auth(self.auth_token.expose_secret()).send().await?;
        if response.status().is_success() {
            let body = response.json::<UserPreferences>().await?;
            Ok(body)
        } else {
            let error = map_response_to_error(response).await.unwrap();
            Err(error)
        }
    }
}

async fn map_response_to_error(response: reqwest::Response) -> Option<Error> {
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return None;
    }

    let service_error = response.json::<ServiceError>().await.expect("service error should follow schema");
    if status.is_client_error() {
        match status {
            StatusCode::UNAUTHORIZED => Some(Error::Unauthorized(service_error)),
            StatusCode::FORBIDDEN => Some(Error::Forbidden(service_error)),
            StatusCode::NOT_FOUND => Some(Error::NotFound(service_error)),
            _ => Some(Error::BadRequest(service_error)),
        }
    } else {
        match status {
            StatusCode::SERVICE_UNAVAILABLE => Some(Error::ServiceUnavailable(service_error)),
            _ => Some(Error::InternalServerError(service_error)),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad request: {0}")]
    BadRequest(ServiceError),
    #[error("unauthorized: {0}")]
    Unauthorized(ServiceError),
    #[error("forbidden: {0}")]
    Forbidden(ServiceError),
    #[error("not found: {0}")]
    NotFound(ServiceError),
    #[error("internal server error: {0}")]
    InternalServerError(ServiceError),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(ServiceError),
    #[error("request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceError {
    pub message: String,
    pub errors: Vec<String>,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
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
