use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use derive_builder::Builder;
use fastwebsockets::FragmentCollectorRead;
use serde_with::{DisplayFromStr, PickFirst, serde_as};
pub use subscription::Command as SubscriptionCommand;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use crate::websocket::WebSocket;

pub mod admin;
pub mod level_one_equities;
pub mod subscription;

type ReadHalf = fastwebsockets::FragmentCollectorRead<tokio::io::ReadHalf<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>>;
type WriteHalf = fastwebsockets::WebSocketWrite<tokio::io::WriteHalf<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>>;

pub struct SchwabStreamerReadHalf {
    read_half: ReadHalf,
    sender: mpsc::Sender<fastwebsockets::Frame<'static>>,
}

impl SchwabStreamerReadHalf {
    pub async fn recv(
        &mut self,
    ) -> Result<Option<StreamerResponse>, fastwebsockets::WebSocketError> {
        let mut send_fn = Box::new(|frame| self.sender.send(frame));
        let frame = self.read_half.read_frame(&mut send_fn).await?;
        match frame.opcode {
            fastwebsockets::OpCode::Text => {
                let raw_response: RawStreamerResponse =
                    serde_json::from_slice(&frame.payload).expect("response should be valid json");
                let response = StreamerResponse::from(raw_response);
                Ok(Some(response))
            }
            _ => Ok(None)
        }
    }
}

#[derive(Clone)]
pub struct SchwabStreamerWriteHalf {
    sender: mpsc::Sender<fastwebsockets::Frame<'static>>,
    customer_id: String,
    correlation_id: String,
    channel: String,
    function_id: String,
    request_id: Arc<AtomicU64>,
}

impl SchwabStreamerWriteHalf {
    pub async fn login(
        &self,
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

    pub async fn logout(&self) -> Result<(), fastwebsockets::WebSocketError> {
        let request = StreamerRequest::logout();
        self.send(request).await
    }

    pub async fn send<T: Into<StreamerRequest>>(
        &self,
        request: T,
    ) -> Result<(), fastwebsockets::WebSocketError> {
        let request: StreamerRequest = request.into();
        let request_id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let request = RequestPayload {
            request_id,
            service: request.service,
            command: request.command,
            parameters: request.parameters,
            schwab_client_customer_id: self.customer_id.clone(),
            schwab_client_correlation_id: self.correlation_id.clone(),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        self.sender
            .send(fastwebsockets::Frame::text(
                fastwebsockets::Payload::Owned(serialized.into()),
            ))
            .await.unwrap();
        Ok(())
    }
}

pub struct FrameSender {
    receiver: mpsc::Receiver<fastwebsockets::Frame<'static>>,
    write_half: WriteHalf,
}

impl FrameSender {
    pub fn run(mut self) -> (tokio::task::JoinHandle<()>, CancellationToken) {
        let token = CancellationToken::new();
        let cloned_token = token.clone();
        let handle = tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    recv = self.receiver.recv() => {
                        if let Some(frame) = recv {
                            self.write_half.write_frame(frame).await.unwrap();
                        } else {
                            break;
                        }
                    }
                    _ = cloned_token.cancelled() => {
                        break;
                    }
                }
            }
        });

        (handle, token)
    }
}

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

    pub fn split(self) -> (SchwabStreamerReadHalf, SchwabStreamerWriteHalf, FrameSender) {
        let (tx, rx) = mpsc::channel::<fastwebsockets::Frame<'static>>(100);
        let (read_half, write_half) = self.websocket.split(tokio::io::split);

        let reader = SchwabStreamerReadHalf {
            read_half: FragmentCollectorRead::new(read_half),
            sender: tx.clone(),
        };

        let writer = SchwabStreamerWriteHalf {
            sender: tx,
            customer_id: self.customer_id,
            correlation_id: self.correlation_id,
            channel: self.channel,
            function_id: self.function_id,
            request_id: Arc::new(AtomicU64::new(self.request_id)),
        };

        let frame_sender = FrameSender {
            receiver: rx,
            write_half: write_half,
        };

        (reader, writer, frame_sender)
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

    pub async fn recv(
        &mut self,
    ) -> Result<StreamerResponse, fastwebsockets::WebSocketError> {
        loop {
            let frame = self.websocket.read_frame().await?;
            if frame.opcode == fastwebsockets::OpCode::Text {
                let raw_response: RawStreamerResponse =
                    serde_json::from_slice(&frame.payload).expect("response should be valid json");
                let response = StreamerResponse::from(raw_response);
                return Ok(response)
            }
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
    pub request_id: u64,
    pub service: Service,
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    pub timestamp: u64,
    pub command: StreamerCommand,
    #[serde(rename = "SchwabClientCorrelId")]
    pub schwab_client_correlation_id: String,
    pub content: ResponseContent,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResponseContent {
    pub code: ResponseCode,
    #[serde(rename = "msg")]
    pub message: String,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Heartbeat {
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    pub heartbeat: u64,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
struct RawDataPayload {
    service: Service,
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    timestamp: u64,
    command: StreamerCommand,
    content: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct DataPayload {
    pub service: Service,
    pub timestamp: u64,
    pub command: SubscriptionCommand,
    pub content: serde_json::Value,
}

impl From<RawDataPayload> for DataPayload {
    fn from(payload: RawDataPayload) -> Self {
        DataPayload {
            service: payload.service,
            timestamp: payload.timestamp,
            command: match payload.command {
                StreamerCommand::Subs => SubscriptionCommand::Subscribe,
                StreamerCommand::Add => SubscriptionCommand::Add,
                StreamerCommand::Unsubs => SubscriptionCommand::Unsubscribe,
                StreamerCommand::View => SubscriptionCommand::View,
                _ => unreachable!(),
            },
            content: transform_keys_for_service(payload.service, payload.content),
        }
    }
}

fn transform_keys_for_service(service: Service, content: serde_json::Value) -> serde_json::Value {
    match service {
        Service::LevelOneEquities => transform_keys::<level_one_equities::Field>(content),
        _ => content,
    }
}

fn transform_keys<T: std::fmt::Display + TryFrom<u8, Error: std::fmt::Debug>>(
    content: serde_json::Value,
) -> serde_json::Value {
    let content = content
        .as_array()
        .expect("data content should be an array")
        .into_iter()
        .map(|item| {
            let map = item
                .as_object()
                .expect("data item should be an object")
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.parse::<u8>()
                            .map(T::try_from)
                            .map(|field| {
                                field.expect("data key should be a valid field").to_string()
                            })
                            .unwrap_or(k.to_string()),
                        v.clone(),
                    )
                })
                .collect();
            serde_json::Value::Object(map)
        })
        .collect();
    serde_json::Value::Array(content)
}

#[derive(Debug, Clone, serde::Deserialize)]
enum RawStreamerResponse {
    #[serde(rename = "response")]
    Response(Vec<ResponsePayload>),
    #[serde(rename = "notify")]
    Notify(Vec<Heartbeat>),
    #[serde(rename = "data")]
    Data(Vec<RawDataPayload>),
}

#[derive(Debug, Clone)]
pub enum StreamerResponse {
    Response(Vec<ResponsePayload>),
    Notify(Vec<Heartbeat>),
    Data(Vec<DataPayload>),
}

impl From<RawStreamerResponse> for StreamerResponse {
    fn from(response: RawStreamerResponse) -> Self {
        match response {
            RawStreamerResponse::Response(responses) => StreamerResponse::Response(responses),
            RawStreamerResponse::Notify(heartbeats) => StreamerResponse::Notify(heartbeats),
            RawStreamerResponse::Data(data) => {
                StreamerResponse::Data(data.into_iter().map(|data| data.into()).collect())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Service {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StreamerCommand {
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
pub enum ResponseCode {
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
