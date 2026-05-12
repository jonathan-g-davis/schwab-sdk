use derive_builder::Builder;
use serde_with::{serde_as, DisplayFromStr, PickFirst};
pub use subscription::Command;

pub mod admin;
pub mod level_one_equities;
pub mod subscription;

type WebSocket =
    fastwebsockets::FragmentCollector<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>;

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct SchwabStreamer {
    websocket: WebSocket,
    customer_id: String,
    correlation_id: String,
    channel: String,
    function_id: String,
    #[builder(default = "0")]
    request_id: u64,
}

impl SchwabStreamer {
    pub(crate) fn builder() -> SchwabStreamerBuilder {
        SchwabStreamerBuilder::default()
    }

    pub async fn login(
        &mut self,
        auth_token: String,
    ) -> Result<(), fastwebsockets::WebSocketError> {
        let request = StreamerRequest::login()
            .authorization(auth_token)
            .schwab_client_channel(self.channel.clone())
            .schwab_client_function_id(self.function_id.clone())
            .build()
            .unwrap();
        self.send(request).await
    }

    pub async fn logout(&mut self) -> Result<(), fastwebsockets::WebSocketError> {
        let request = StreamerRequest::logout();
        self.send(request).await
    }

    pub async fn send<T: Into<StreamerRequest>>(
        &mut self,
        request: T,
    ) -> Result<(), fastwebsockets::WebSocketError> {
        let request: StreamerRequest = request.into();
        let request = RequestPayload {
            request_id: self.request_id,
            service: request.service,
            command: request.command,
            parameters: request.parameters,
            schwab_client_customer_id: self.customer_id.clone(),
            schwab_client_correlation_id: self.correlation_id.clone(),
        };
        self.request_id += 1;

        let serialized = serde_json::to_string(&request).unwrap();
        self.websocket
            .write_frame(fastwebsockets::Frame::text(
                fastwebsockets::Payload::Borrowed(serialized.as_bytes()),
            ))
            .await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Option<StreamerResponse>, fastwebsockets::WebSocketError> {
        let frame = self.websocket.read_frame().await?;
        if frame.opcode == fastwebsockets::OpCode::Text {
            Ok(serde_json::from_slice(&frame.payload).expect("response should be valid json"))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct RequestPayload {
    #[serde(rename = "requestid")]
    request_id: u64,
    #[serde(rename = "service")]
    service: Service,
    #[serde(rename = "command")]
    command: StreamerCommand,
    #[serde(rename = "parameters")]
    parameters: serde_json::Value,
    #[serde(rename = "SchwabClientCustomerId")]
    schwab_client_customer_id: String,
    #[serde(rename = "SchwabClientCorrelId")]
    schwab_client_correlation_id: String,
}

pub struct StreamerRequest {
    service: Service,
    command: StreamerCommand,
    parameters: serde_json::Value,
}

impl StreamerRequest {
    pub fn login() -> admin::LoginBuilder {
        admin::LoginBuilder::default()
    }

    pub fn logout() -> admin::Logout {
        admin::Logout
    }

    pub fn equities() -> subscription::SubscriptionBuilder<level_one_equities::Field> {
        subscription::SubscriptionBuilder::default()
    }
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResponsePayload {
    #[serde(rename = "requestid")]
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    request_id: u64,
    service: Service,
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    timestamp: u64,
    command: StreamerCommand,
    #[serde(rename = "SchwabClientCorrelId")]
    schwab_client_correlation_id: String,
    content: ResponseContent,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResponseContent {
    code: ResponseCode,
    #[serde(rename = "msg")]
    message: String,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Heartbeat {
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    heartbeat: u64,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DataPayload {
    service: Service,
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    timestamp: u64,
    command: StreamerCommand,
    content: serde_json::Value,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub enum StreamerResponse {
    #[serde(rename = "response")]
    Response(Vec<ResponsePayload>),
    #[serde(rename = "notify")]
    Notify(Vec<Heartbeat>),
    #[serde(rename = "data")]
    Data(Vec<DataPayload>),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum Service {
    #[serde(rename = "ADMIN")]
    Admin,
    #[serde(rename = "LEVELONE_EQUITIES")]
    LevelOneEquities,
    #[serde(rename = "LEVELONE_OPTIONS")]
    LevelOneOptions,
    #[serde(rename = "LEVELONE_FUTURES")]
    LevelOneFutures,
    #[serde(rename = "LEVELONE_FUTURES_OPTIONS")]
    LevelOneFuturesOptions,
    #[serde(rename = "LEVELONE_FOREX")]
    LevelOneForex,
    #[serde(rename = "NYSE_BOOK")]
    NyseBook,
    #[serde(rename = "NASDAQ_BOOK")]
    NasdaqBook,
    #[serde(rename = "OPTIONS_BOOK")]
    OptionsBook,
    #[serde(rename = "CHART_EQUITY")]
    ChartEquity,
    #[serde(rename = "CHART_FUTURES")]
    ChartFutures,
    #[serde(rename = "SCREENER_EQUITY")]
    ScreenerEquity,
    #[serde(rename = "SCREENER_OPTION")]
    ScreenerOption,
    #[serde(rename = "ACCT_ACTIVITY")]
    AccountActivity,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum StreamerCommand {
    #[serde(rename = "LOGIN")]
    Login,
    #[serde(rename = "SUBS")]
    Subs,
    #[serde(rename = "ADD")]
    Add,
    #[serde(rename = "UNSUBS")]
    Unsubs,
    #[serde(rename = "VIEW")]
    View,
    #[serde(rename = "LOGOUT")]
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde_repr::Deserialize_repr)]
#[repr(u8)]
enum ResponseCode {
    Ok = 0,
    LoginDenied = 3,
    UnknownFailure = 9,
    ServiceNotAvailable = 11,
    CloseConnection = 12,
    ReachedSymbolLimit = 19,
    StreamConnNotFound,
    BadCommandFormat,
    FailedCommandSubs,
    FailedCommandUnsubs,
    FailedCommandAdd,
    FailedCommandView,
    SucceededCommandSubs,
    SucceededCommandUnsubs,
    SucceededCommandAdd,
    SucceededCommandView,
    StopStreaming,
}
