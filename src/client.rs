use http::{StatusCode, Uri};
use secrecy::{ExposeSecret, SecretString};

use crate::{SchwabStreamer, websocket};

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
        let response = self
            .client
            .get(url)
            .bearer_auth(self.auth_token.expose_secret())
            .send()
            .await?;
        if response.status().is_success() {
            let body = response.json::<UserPreferences>().await?;
            Ok(body)
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

async fn map_response_to_error(response: reqwest::Response) -> Error {
    let status = response.status();
    let retry_after = parse_retry_after(response.headers());
    let service_error = match response.json::<ServiceError>().await {
        Ok(body) => body,
        Err(source) => {
            return Error::Decode {
                context: format!("service error body (status {status})"),
                reason: source.to_string(),
            };
        }
    };
    match status {
        StatusCode::UNAUTHORIZED => Error::Unauthorized(service_error),
        StatusCode::FORBIDDEN => Error::Forbidden(service_error),
        StatusCode::NOT_FOUND => Error::NotFound(service_error),
        StatusCode::TOO_MANY_REQUESTS => Error::RateLimited {
            retry_after,
            body: service_error,
        },
        StatusCode::SERVICE_UNAVAILABLE => Error::ServiceUnavailable(service_error),
        s if s.is_server_error() => Error::InternalServerError(service_error),
        _ => Error::BadRequest(service_error),
    }
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<std::time::Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    value
        .parse::<u64>()
        .ok()
        .map(std::time::Duration::from_secs)
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
    #[error("rate limited: {body}")]
    RateLimited {
        retry_after: Option<std::time::Duration>,
        body: ServiceError,
    },
    #[error("internal server error: {0}")]
    InternalServerError(ServiceError),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(ServiceError),
    #[error("request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("websocket error: {0}")]
    WebSocket(#[from] websocket::WebSocketError),
    #[error("streamer transport: {0}")]
    Streamer(#[from] fastwebsockets::WebSocketError),
    #[error("decode {context}: {reason}")]
    Decode { context: String, reason: String },
    #[error("build: {0}")]
    Build(String),
    #[error("outbound frame channel closed")]
    ChannelClosed,
    #[error("missing user preference field: {0}")]
    MissingPreference(&'static str),
    #[error("invalid uri: {0}")]
    InvalidUri(String),
}

impl Error {
    /// Schwab-specific retry classification. Returns `true` for transient
    /// failures (network, 5xx, 429) where the same request can be safely
    /// retried by the caller. Returns `false` for terminal failures
    /// (4xx other than 429, decode errors, build errors).
    ///
    /// `schwab-rs` does not implement retry itself; this method exists so
    /// downstream consumers can wire in `backon` or another retry policy.
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::RateLimited { .. }
            | Error::InternalServerError(_)
            | Error::ServiceUnavailable(_) => true,
            Error::RequestFailed(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            Error::WebSocket(_) | Error::Streamer(_) | Error::ChannelClosed => true,
            Error::BadRequest(_)
            | Error::Unauthorized(_)
            | Error::Forbidden(_)
            | Error::NotFound(_)
            | Error::Decode { .. }
            | Error::Build(_)
            | Error::MissingPreference(_)
            | Error::InvalidUri(_) => false,
        }
    }

    /// `Retry-After` duration parsed from a 429 response, when present.
    /// Always `None` for non-rate-limited errors.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Error::RateLimited { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
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
