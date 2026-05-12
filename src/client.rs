use std::collections::HashMap;

use http::{StatusCode, Uri};
use secrecy::{ExposeSecret, SecretString};

use crate::websocket;

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
            let error = map_response_to_error(response).await.unwrap();
            Err(error)
        }
    }

    pub async fn connect_streamer(&self) -> Result<SchwabStreamer> {
        let user_preferences = self.get_user_preferences().await?;
        let streamer_info = user_preferences
            .streamer_info
            .into_iter()
            .next()
            .expect("streamer info should be present");
        let uri = streamer_info
            .streamer_socket_url
            .parse::<Uri>()
            .expect("streamer socket url should be a valid uri");
        let websocket = websocket::connect(uri).await?;
        Ok(SchwabStreamer {
            websocket,
            auth_token: self.auth_token.clone(),
            customer_id: streamer_info.schwab_client_customer_id,
            correlation_id: streamer_info.schwab_client_correlation_id,
            channel: streamer_info.schwab_client_channel,
            function_id: streamer_info.schwab_client_function_id,
            request_id: 0,
        })
    }
}

pub struct SchwabStreamer {
    websocket: fastwebsockets::FragmentCollector<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>,
    auth_token: SecretString,
    customer_id: String,
    correlation_id: String,
    channel: String,
    function_id: String,
    request_id: u64,
}

impl SchwabStreamer {
    pub async fn login(&mut self) -> Result<()> {
        let request = StreamerRequest {
            request_id: self.request_id,
            service: "ADMIN".to_string(),
            command: "LOGIN".to_string(),
            schwab_client_customer_id: self.customer_id.to_string(),
            schwab_client_correlation_id: self.correlation_id.to_string(),
            parameters: Login {
                authorization: self.auth_token.expose_secret().to_string(),
                schwab_client_channel: self.channel.to_string(),
                schwab_client_function_id: self.function_id.to_string(),
            },
        };

        self.request_id += 1;

        self.websocket
            .write_frame(fastwebsockets::Frame::text(
                fastwebsockets::Payload::Borrowed(
                    serde_json::to_string(&request).unwrap().as_bytes(),
                ),
            ))
            .await
            .expect("failed to write frame");

        Ok(())
    }

    pub async fn logout(&mut self) -> Result<()> {
        let request = StreamerRequest {
            request_id: self.request_id,
            service: "ADMIN".to_string(),
            command: "LOGOUT".to_string(),
            schwab_client_customer_id: self.customer_id.to_string(),
            schwab_client_correlation_id: self.correlation_id.to_string(),
            parameters: HashMap::<String, String>::new(),
        };

        self.request_id += 1;

        self.websocket
            .write_frame(fastwebsockets::Frame::text(
                fastwebsockets::Payload::Borrowed(
                    serde_json::to_string(&request).unwrap().as_bytes(),
                ),
            ))
            .await.expect("failed to write frame");

        Ok(())
    }

    pub async fn read_frame(&mut self) -> Result<Option<String>> {
        let frame = self
            .websocket
            .read_frame()
            .await
            .expect("failed to read frame");
        if frame.opcode == fastwebsockets::OpCode::Text {
            let text =
                String::from_utf8(frame.payload.to_vec()).expect("frame should be valid utf-8");
            Ok(Some(text))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct StreamerRequest<T> {
    #[serde(rename = "requestid")]
    request_id: u64,
    #[serde(rename = "service")]
    service: String,
    #[serde(rename = "command")]
    command: String,
    #[serde(rename = "SchwabClientCustomerId")]
    schwab_client_customer_id: String,
    #[serde(rename = "SchwabClientCorrelId")]
    schwab_client_correlation_id: String,
    #[serde(rename = "parameters")]
    parameters: T,
}

#[derive(Debug, Clone, serde::Serialize)]
struct Login {
    #[serde(rename = "Authorization")]
    authorization: String,
    #[serde(rename = "SchwabClientChannel")]
    schwab_client_channel: String,
    #[serde(rename = "SchwabClientFunctionId")]
    schwab_client_function_id: String,
}

async fn map_response_to_error(response: reqwest::Response) -> Option<Error> {
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return None;
    }

    let service_error = response
        .json::<ServiceError>()
        .await
        .expect("service error should follow schema");
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
    #[error("websocket error: {0}")]
    WebSocket(#[from] websocket::WebSocketError),
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
