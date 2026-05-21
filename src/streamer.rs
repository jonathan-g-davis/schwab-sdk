use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use derive_builder::Builder;
use fastwebsockets::FragmentCollectorRead;
use serde_with::{DisplayFromStr, PickFirst, serde_as};
pub use subscription::Command as SubscriptionCommand;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use crate::error::{Error, Result};
use crate::model::{AuthToken, CustomerId};
use crate::websocket::WebSocket;

pub mod account_activity;
pub mod admin;
pub mod book;
pub mod chart;
pub mod level_one;
pub mod screener;
pub mod subscription;

type ReadHalf = fastwebsockets::FragmentCollectorRead<tokio::io::ReadHalf<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>>;
type WriteHalf = fastwebsockets::WebSocketWrite<tokio::io::WriteHalf<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>>;

pub struct SchwabStreamerReadHalf {
    read_half: ReadHalf,
    sender: mpsc::Sender<fastwebsockets::Frame<'static>>,
}

impl SchwabStreamerReadHalf {
    pub async fn recv(&mut self) -> Result<StreamerResponse> {
        let mut send_fn = Box::new(|frame| self.sender.send(frame));
        loop {
            let frame = self.read_half.read_frame(&mut send_fn).await?;
            if frame.opcode == fastwebsockets::OpCode::Text {
                let raw_response: RawStreamerResponse = serde_json::from_slice(&frame.payload)
                    .map_err(|e| Error::Decode {
                        context: "streamer response frame".to_string(),
                        reason: e.to_string(),
                    })?;
                return StreamerResponse::try_from(raw_response);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchwabStreamerWriteHalf {
    sender: mpsc::Sender<fastwebsockets::Frame<'static>>,
    customer_id: CustomerId,
    correlation_id: String,
    channel: String,
    function_id: String,
    request_id: Arc<AtomicU64>,
}

impl SchwabStreamerWriteHalf {
    pub async fn login(&self, auth_token: AuthToken) -> Result<()> {
        let request = StreamerRequest::login()
            .authorization(auth_token)
            .schwab_client_channel(self.channel.clone())
            .schwab_client_function_id(self.function_id.clone())
            .build()
            .map_err(|e| Error::Build(e.to_string()))?;
        self.send(request).await
    }

    pub async fn logout(&self) -> Result<()> {
        self.send(StreamerRequest::logout()).await
    }

    pub async fn send<T: Into<StreamerRequest>>(&self, request: T) -> Result<()> {
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

        let serialized = serde_json::to_string(&request).map_err(|e| Error::Encode {
            context: "streamer request envelope".to_string(),
            reason: e.to_string(),
        })?;
        self.sender
            .send(fastwebsockets::Frame::text(
                fastwebsockets::Payload::Owned(serialized.into()),
            ))
            .await
            .map_err(|_| Error::ChannelClosed)?;
        Ok(())
    }
}

pub struct FrameSender {
    receiver: mpsc::Receiver<fastwebsockets::Frame<'static>>,
    write_half: WriteHalf,
}

impl FrameSender {
    /// Spawn the outbound frame pump. The returned `JoinHandle` resolves to
    /// `Ok(())` on graceful shutdown (channel closed or cancellation), or
    /// `Err` if the underlying websocket write fails. Callers are expected
    /// to supervise the handle and treat a write failure as a disconnect.
    pub fn run(mut self) -> (tokio::task::JoinHandle<Result<()>>, CancellationToken) {
        let token = CancellationToken::new();
        let cloned_token = token.clone();
        let handle = tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    recv = self.receiver.recv() => {
                        match recv {
                            Some(frame) => self.write_half.write_frame(frame).await?,
                            None => break,
                        }
                    }
                    _ = cloned_token.cancelled() => {
                        break;
                    }
                }
            }
            Ok(())
        });

        (handle, token)
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct SchwabStreamer {
    websocket: WebSocket,
    customer_id: CustomerId,
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
            write_half,
        };

        (reader, writer, frame_sender)
    }

    pub async fn login(&mut self, auth_token: AuthToken) -> Result<()> {
        let request = StreamerRequest::login()
            .authorization(auth_token)
            .schwab_client_channel(self.channel.clone())
            .schwab_client_function_id(self.function_id.clone())
            .build()
            .map_err(|e| Error::Build(e.to_string()))?;
        self.send(request).await
    }

    pub async fn logout(&mut self) -> Result<()> {
        self.send(StreamerRequest::logout()).await
    }

    pub async fn send<T: Into<StreamerRequest>>(&mut self, request: T) -> Result<()> {
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

        let serialized = serde_json::to_string(&request).map_err(|e| Error::Encode {
            context: "streamer request envelope".to_string(),
            reason: e.to_string(),
        })?;
        self.websocket
            .write_frame(fastwebsockets::Frame::text(
                fastwebsockets::Payload::Borrowed(serialized.as_bytes()),
            ))
            .await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<StreamerResponse> {
        loop {
            let frame = self.websocket.read_frame().await?;
            if frame.opcode == fastwebsockets::OpCode::Text {
                let raw_response: RawStreamerResponse = serde_json::from_slice(&frame.payload)
                    .map_err(|e| Error::Decode {
                        context: "streamer response frame".to_string(),
                        reason: e.to_string(),
                    })?;
                return StreamerResponse::try_from(raw_response);
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
    schwab_client_customer_id: CustomerId,
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

    pub fn equities() -> subscription::SubscriptionBuilder<level_one::equities::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn options() -> subscription::SubscriptionBuilder<level_one::options::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn futures() -> subscription::SubscriptionBuilder<level_one::futures::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn futures_options()
    -> subscription::SubscriptionBuilder<level_one::futures_options::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn forex() -> subscription::SubscriptionBuilder<level_one::forex::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn nyse_book() -> subscription::SubscriptionBuilder<book::nyse::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn nasdaq_book() -> subscription::SubscriptionBuilder<book::nasdaq::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn options_book() -> subscription::SubscriptionBuilder<book::options::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn chart_equity() -> subscription::SubscriptionBuilder<chart::equity::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn chart_futures() -> subscription::SubscriptionBuilder<chart::futures::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn screener_equity() -> subscription::SubscriptionBuilder<screener::equity::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn screener_option() -> subscription::SubscriptionBuilder<screener::option::Field> {
        subscription::SubscriptionBuilder::default()
    }

    pub fn account_activity() -> subscription::SubscriptionBuilder<account_activity::Field> {
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

/// One element of a `data` array on a streamer frame, already decoded into a
/// service-specific typed shape.
#[derive(Debug, Clone)]
pub struct DataPayload {
    pub service: Service,
    pub timestamp: u64,
    pub command: SubscriptionCommand,
    pub content: DataContent,
}

/// Typed content per streamer service.
///
/// Each variant corresponds to a service whose payload `schwab-rs` decodes
/// into typed fields. Services not yet typed land in [`DataContent::Raw`]
/// with Schwab's numeric-keyed JSON object preserved, so callers can still
/// destructure them by hand until a typed variant is added.
#[derive(Debug, Clone)]
pub enum DataContent {
    LevelOneEquities(Vec<level_one::equities::Content>),
    LevelOneOptions(Vec<level_one::options::Content>),
    LevelOneFutures(Vec<level_one::futures::Content>),
    LevelOneFuturesOptions(Vec<level_one::futures_options::Content>),
    LevelOneForex(Vec<level_one::forex::Content>),
    NyseBook(Vec<book::Content>),
    NasdaqBook(Vec<book::Content>),
    OptionsBook(Vec<book::Content>),
    ChartEquity(Vec<chart::equity::Content>),
    ChartFutures(Vec<chart::futures::Content>),
    ScreenerEquity(Vec<screener::Content>),
    ScreenerOption(Vec<screener::Content>),
    AccountActivity(Vec<account_activity::Content>),
    /// Untyped fallback for services that don't have a typed variant yet.
    /// The inner value is the raw `content` array from Schwab with numeric
    /// field keys remapped to their snake_case names where the streamer
    /// knows the field set, and left numeric otherwise.
    Raw(serde_json::Value),
}

impl TryFrom<RawDataPayload> for DataPayload {
    type Error = Error;

    fn try_from(payload: RawDataPayload) -> Result<Self> {
        let command = match payload.command {
            StreamerCommand::Subs => SubscriptionCommand::Subscribe,
            StreamerCommand::Add => SubscriptionCommand::Add,
            StreamerCommand::Unsubs => SubscriptionCommand::Unsubscribe,
            StreamerCommand::View => SubscriptionCommand::View,
            other => {
                return Err(Error::Decode {
                    context: "data payload command".to_string(),
                    reason: format!("unexpected command {other:?}"),
                });
            }
        };
        let content = decode_service_content(payload.service, payload.content)?;
        Ok(DataPayload {
            service: payload.service,
            timestamp: payload.timestamp,
            command,
            content,
        })
    }
}

fn decode_service_content(service: Service, content: serde_json::Value) -> Result<DataContent> {
    match service {
        Service::LevelOneEquities => {
            let remapped = transform_keys::<level_one::equities::Field>(content)?;
            Ok(DataContent::LevelOneEquities(
                level_one::equities::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneOptions => {
            let remapped = transform_keys::<level_one::options::Field>(content)?;
            Ok(DataContent::LevelOneOptions(
                level_one::options::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneFutures => {
            let remapped = transform_keys::<level_one::futures::Field>(content)?;
            Ok(DataContent::LevelOneFutures(
                level_one::futures::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneFuturesOptions => {
            let remapped = transform_keys::<level_one::futures_options::Field>(content)?;
            Ok(DataContent::LevelOneFuturesOptions(
                level_one::futures_options::Content::decode_batch(remapped)?,
            ))
        }
        Service::LevelOneForex => {
            let remapped = transform_keys::<level_one::forex::Field>(content)?;
            Ok(DataContent::LevelOneForex(
                level_one::forex::Content::decode_batch(remapped)?,
            ))
        }
        Service::NyseBook => {
            let remapped = transform_keys::<book::nyse::Field>(content)?;
            Ok(DataContent::NyseBook(book::nyse::decode_batch(remapped)?))
        }
        Service::NasdaqBook => {
            let remapped = transform_keys::<book::nasdaq::Field>(content)?;
            Ok(DataContent::NasdaqBook(book::nasdaq::decode_batch(
                remapped,
            )?))
        }
        Service::OptionsBook => {
            let remapped = transform_keys::<book::options::Field>(content)?;
            Ok(DataContent::OptionsBook(book::options::decode_batch(
                remapped,
            )?))
        }
        Service::ChartEquity => {
            let remapped = transform_keys::<chart::equity::Field>(content)?;
            Ok(DataContent::ChartEquity(
                chart::equity::Content::decode_batch(remapped)?,
            ))
        }
        Service::ChartFutures => {
            let remapped = transform_keys::<chart::futures::Field>(content)?;
            Ok(DataContent::ChartFutures(
                chart::futures::Content::decode_batch(remapped)?,
            ))
        }
        Service::ScreenerEquity => {
            let remapped = transform_keys::<screener::equity::Field>(content)?;
            Ok(DataContent::ScreenerEquity(screener::equity::decode_batch(
                remapped,
            )?))
        }
        Service::ScreenerOption => {
            let remapped = transform_keys::<screener::option::Field>(content)?;
            Ok(DataContent::ScreenerOption(screener::option::decode_batch(
                remapped,
            )?))
        }
        Service::AccountActivity => {
            let remapped = transform_keys::<account_activity::Field>(content)?;
            Ok(DataContent::AccountActivity(
                account_activity::Content::decode_batch(remapped)?,
            ))
        }
        _ => Ok(DataContent::Raw(content)),
    }
}

fn transform_keys<T: std::fmt::Display + TryFrom<u8>>(
    content: serde_json::Value,
) -> Result<serde_json::Value> {
    let array = content.as_array().ok_or_else(|| Error::Decode {
        context: "data payload content".to_string(),
        reason: "expected array".to_string(),
    })?;
    let mut out = Vec::with_capacity(array.len());
    for item in array {
        let object = item.as_object().ok_or_else(|| Error::Decode {
            context: "data payload item".to_string(),
            reason: "expected object".to_string(),
        })?;
        let mut map = serde_json::Map::with_capacity(object.len());
        for (k, v) in object {
            // Field-number keys get remapped to their name; everything else
            // (e.g. "key", "delayed", "assetMainType") passes through. An
            // unknown numeric discriminant is forward-compatibility: keep
            // the raw key so the consumer can still see the field.
            let mapped = match k.parse::<u8>() {
                Ok(n) => T::try_from(n)
                    .map(|field| field.to_string())
                    .unwrap_or_else(|_| k.clone()),
                Err(_) => k.clone(),
            };
            map.insert(mapped, v.clone());
        }
        out.push(serde_json::Value::Object(map));
    }
    Ok(serde_json::Value::Array(out))
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

impl TryFrom<RawStreamerResponse> for StreamerResponse {
    type Error = Error;

    fn try_from(response: RawStreamerResponse) -> Result<Self> {
        Ok(match response {
            RawStreamerResponse::Response(responses) => StreamerResponse::Response(responses),
            RawStreamerResponse::Notify(heartbeats) => StreamerResponse::Notify(heartbeats),
            RawStreamerResponse::Data(data) => {
                let converted = data
                    .into_iter()
                    .map(DataPayload::try_from)
                    .collect::<Result<Vec<DataPayload>>>()?;
                StreamerResponse::Data(converted)
            }
        })
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

#[cfg(test)]
mod parser_tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn parse(raw: &str) -> Result<StreamerResponse> {
        let raw_response: RawStreamerResponse =
            serde_json::from_slice(raw.as_bytes()).map_err(|e| Error::Decode {
                context: "test fixture".to_string(),
                reason: e.to_string(),
            })?;
        StreamerResponse::try_from(raw_response)
    }

    #[test]
    fn parses_login_success_response() {
        let frame = r#"{
            "response": [{
                "service": "ADMIN",
                "command": "LOGIN",
                "requestid": "1",
                "SchwabClientCorrelId": "5be0b7e7-5b8b-4fd3-9bed-7f49106cfe96",
                "timestamp": 1669828276886,
                "content": { "code": 0, "msg": "server=s0166bdv-1;status=PN" }
            }]
        }"#;
        match parse(frame).unwrap() {
            StreamerResponse::Response(responses) => {
                assert_eq!(responses.len(), 1);
                let r = &responses[0];
                assert_eq!(r.service, Service::Admin);
                assert_eq!(r.command, StreamerCommand::Login);
                assert_eq!(r.request_id, 1);
                assert_eq!(r.content.code, ResponseCode::Ok);
                assert!(r.content.message.contains("status=PN"));
            }
            other => panic!("expected Response, got {other:?}"),
        }
    }

    #[test]
    fn parses_login_denied_response() {
        let frame = r#"{
            "response": [{
                "service": "ADMIN",
                "command": "LOGIN",
                "requestid": "1",
                "SchwabClientCorrelId": "x",
                "timestamp": 1669828982588,
                "content": { "code": 3, "msg": "Login Denied.: token is invalid or has expired." }
            }]
        }"#;
        let StreamerResponse::Response(responses) = parse(frame).unwrap() else {
            panic!("expected Response");
        };
        assert_eq!(responses[0].content.code, ResponseCode::LoginDenied);
    }

    #[test]
    fn parses_heartbeat_notify() {
        let frame = r#"{"notify":[{"heartbeat":"1668715930582"}]}"#;
        let StreamerResponse::Notify(heartbeats) = parse(frame).unwrap() else {
            panic!("expected Notify");
        };
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].heartbeat, 1668715930582);
    }

    #[test]
    fn parses_level_one_equities_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_EQUITIES",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [
                    {
                        "key": "SCHW",
                        "delayed": false,
                        "assetMainType": "EQUITY",
                        "assetSubType": "COE",
                        "cusip": "808513105",
                        "1": 76.08, "2": 76.49, "3": 76.44,
                        "4": 3, "5": 1, "8": 5414735, "10": 76.47
                    },
                    {
                        "key": "AAPL",
                        "delayed": false,
                        "assetMainType": "EQUITY",
                        "assetSubType": "COE",
                        "cusip": "037833100",
                        "1": 183.75, "2": 183.8, "3": 183.8,
                        "4": 1, "5": 2, "8": 163224109, "10": 187
                    }
                ]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        assert_eq!(data.len(), 1);
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneEquities);
        assert_eq!(payload.timestamp, 1714949592301);
        assert_eq!(payload.command, SubscriptionCommand::Subscribe);

        let DataContent::LevelOneEquities(items) = &payload.content else {
            panic!("expected LevelOneEquities, got {:?}", payload.content);
        };
        assert_eq!(items.len(), 2);

        let schw = &items[0];
        assert_eq!(schw.key, "SCHW");
        assert!(!schw.delayed);
        assert_eq!(schw.cusip.as_deref(), Some("808513105"));
        assert_eq!(schw.bid_price, Some(dec!(76.08)));
        assert_eq!(schw.ask_price, Some(dec!(76.49)));
        assert_eq!(schw.last_price, Some(dec!(76.44)));
        assert_eq!(schw.bid_size, Some(3));
        assert_eq!(schw.ask_size, Some(1));
        assert_eq!(schw.total_volume, Some(5414735));
        assert_eq!(schw.high_price, Some(dec!(76.47)));
        // Fields not present on the wire stay None.
        assert_eq!(schw.low_price, None);
        assert_eq!(schw.dividend_yield, None);

        let aapl = &items[1];
        assert_eq!(aapl.key, "AAPL");
        assert_eq!(aapl.bid_price, Some(dec!(183.75)));
        assert_eq!(aapl.last_price, Some(dec!(183.8)));
    }

    #[test]
    fn unknown_numeric_field_does_not_fail_parse() {
        // Schwab adds a new field 99 we haven't typed yet. The remapper
        // should keep the raw "99" key (so it's accessible if anyone drops
        // down to Raw), and the typed struct ignores it via #[serde(default)]
        // and unknown-field tolerance (Deserialize is non-deny by default).
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_EQUITIES",
                "timestamp": 1,
                "command": "SUBS",
                "content": [{
                    "key": "X", "delayed": false,
                    "1": 1.0, "99": "future-field"
                }]
            }]
        }"#;
        let response = parse(frame).expect("forward-compat parse failed");
        let StreamerResponse::Data(data) = response else {
            panic!("expected Data");
        };
        let DataContent::LevelOneEquities(items) = &data[0].content else {
            panic!("expected LevelOneEquities");
        };
        assert_eq!(items[0].bid_price, Some(dec!(1.0)));
    }

    #[test]
    fn parses_level_one_options_data_into_typed_content() {
        // An ATM-ish AAPL call: bid 5.10 / ask 5.20, last 5.15, delta 0.52,
        // gamma 0.04, theta -0.08, vega 0.13, 7 DTE.
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_OPTIONS",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL  240315C00200000",
                    "delayed": false,
                    "assetMainType": "OPTION",
                    "2": 5.10, "3": 5.20, "4": 5.15,
                    "8": 12345, "9": 6789,
                    "20": 200.0, "21": "C", "22": "AAPL",
                    "27": 7, "28": 0.52, "29": 0.04, "30": -0.08, "31": 0.13,
                    "37": 5.15,
                    "48": true
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneOptions);

        let DataContent::LevelOneOptions(items) = &payload.content else {
            panic!("expected LevelOneOptions, got {:?}", payload.content);
        };
        assert_eq!(items.len(), 1);
        let aapl = &items[0];
        assert_eq!(aapl.key, "AAPL  240315C00200000");
        assert_eq!(aapl.bid_price, Some(dec!(5.10)));
        assert_eq!(aapl.ask_price, Some(dec!(5.20)));
        assert_eq!(aapl.last_price, Some(dec!(5.15)));
        assert_eq!(aapl.total_volume, Some(12345));
        assert_eq!(aapl.open_interest, Some(6789));
        assert_eq!(aapl.strike_price, Some(dec!(200.0)));
        assert_eq!(aapl.contract_type.as_deref(), Some("C"));
        assert_eq!(aapl.underlying.as_deref(), Some("AAPL"));
        assert_eq!(aapl.days_to_expiration, Some(7));
        assert_eq!(aapl.delta, Some(dec!(0.52)));
        assert_eq!(aapl.gamma, Some(dec!(0.04)));
        assert_eq!(aapl.theta, Some(dec!(-0.08)));
        assert_eq!(aapl.vega, Some(dec!(0.13)));
        assert_eq!(aapl.mark_price, Some(dec!(5.15)));
        assert_eq!(aapl.is_penny_pilot, Some(true));
        // Fields not on wire stay None.
        assert_eq!(aapl.rho, None);
        assert_eq!(aapl.implied_yield, None);
    }

    #[test]
    fn parses_level_one_futures_data_into_typed_content() {
        // /ESZ24 (E-Mini S&P 500 Dec 2024) with quotes, volume, multiplier.
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_FUTURES",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "/ESZ24",
                    "delayed": false,
                    "1": 5025.25, "2": 5025.50, "3": 5025.25,
                    "4": 12, "5": 9,
                    "8": 1234567, "12": 5050.00, "13": 5005.75,
                    "16": "E-Mini S&P 500 Dec 24",
                    "24": 5025.375,
                    "25": 0.25, "26": 12.50,
                    "30": true, "31": 50.0, "32": true
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneFutures);
        let DataContent::LevelOneFutures(items) = &payload.content else {
            panic!("expected LevelOneFutures, got {:?}", payload.content);
        };
        assert_eq!(items.len(), 1);
        let es = &items[0];
        assert_eq!(es.key, "/ESZ24");
        assert_eq!(es.bid_price, Some(dec!(5025.25)));
        assert_eq!(es.ask_price, Some(dec!(5025.50)));
        assert_eq!(es.last_price, Some(dec!(5025.25)));
        assert_eq!(es.bid_size, Some(12));
        assert_eq!(es.ask_size, Some(9));
        assert_eq!(es.total_volume, Some(1234567));
        assert_eq!(es.high_price, Some(dec!(5050.00)));
        assert_eq!(es.low_price, Some(dec!(5005.75)));
        assert_eq!(es.description.as_deref(), Some("E-Mini S&P 500 Dec 24"));
        assert_eq!(es.mark, Some(dec!(5025.375)));
        assert_eq!(es.tick, Some(dec!(0.25)));
        assert_eq!(es.tick_amount, Some(dec!(12.50)));
        assert_eq!(es.future_is_tradable, Some(true));
        assert_eq!(es.future_multiplier, Some(dec!(50.0)));
        assert_eq!(es.future_is_active, Some(true));
    }

    #[test]
    fn parses_level_one_futures_options_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_FUTURES_OPTIONS",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "./OZCZ23C565",
                    "delayed": false,
                    "1": 12.25, "2": 12.50, "3": 12.375,
                    "4": 5, "5": 7, "8": 234,
                    "18": 1500.5,
                    "19": 12.375, "20": 0.25, "21": 12.50,
                    "22": 50.0,
                    "24": "/ZCZ23", "25": 565.0,
                    "28": "C"
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneFuturesOptions);
        let DataContent::LevelOneFuturesOptions(items) = &payload.content else {
            panic!("expected LevelOneFuturesOptions");
        };
        let item = &items[0];
        assert_eq!(item.key, "./OZCZ23C565");
        assert_eq!(item.bid_price, Some(dec!(12.25)));
        assert_eq!(item.ask_price, Some(dec!(12.50)));
        assert_eq!(item.total_volume, Some(234));
        assert_eq!(item.open_interest, Some(dec!(1500.5))); // double per spec
        assert_eq!(item.mark, Some(dec!(12.375)));
        assert_eq!(item.future_multiplier, Some(dec!(50.0)));
        assert_eq!(item.underlying_symbol.as_deref(), Some("/ZCZ23"));
        assert_eq!(item.strike_price, Some(dec!(565.0)));
        assert_eq!(item.contract_type.as_deref(), Some("C"));
    }

    #[test]
    fn parses_level_one_forex_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_FOREX",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "EUR/USD",
                    "delayed": false,
                    "1": 1.0825, "2": 1.0826, "3": 1.08255,
                    "4": 1000000, "5": 1500000,
                    "10": 1.0850, "11": 1.0810, "12": 1.0820,
                    "14": "Euro/US Dollar",
                    "16": 0.00055, "17": 0.0508,
                    "19": 5,
                    "25": true, "29": 1.08255
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneForex);
        let DataContent::LevelOneForex(items) = &payload.content else {
            panic!("expected LevelOneForex");
        };
        let eur = &items[0];
        assert_eq!(eur.key, "EUR/USD");
        assert_eq!(eur.bid_price, Some(dec!(1.0825)));
        assert_eq!(eur.ask_price, Some(dec!(1.0826)));
        assert_eq!(eur.last_price, Some(dec!(1.08255)));
        assert_eq!(eur.bid_size, Some(1_000_000));
        assert_eq!(eur.ask_size, Some(1_500_000));
        assert_eq!(eur.description.as_deref(), Some("Euro/US Dollar"));
        assert_eq!(eur.percent_change, Some(dec!(0.0508)));
        assert_eq!(eur.digits, Some(5));
        assert_eq!(eur.is_tradable, Some(true));
        assert_eq!(eur.mark, Some(dec!(1.08255)));
    }

    #[test]
    fn parses_nyse_book_data_into_typed_content() {
        // One bid level at 150.00 with two market makers and one ask level
        // at 150.05 with a single market maker.
        let frame = r#"{
            "data": [{
                "service": "NYSE_BOOK",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL",
                    "delayed": false,
                    "1": 1714949592300,
                    "2": [{
                        "0": 150.00,
                        "1": 1000,
                        "2": 2,
                        "3": [
                            {"0": "MM1", "1": 600, "2": 1714949592000},
                            {"0": "MM2", "1": 400, "2": 1714949592100}
                        ]
                    }],
                    "3": [{
                        "0": 150.05,
                        "1": 500,
                        "2": 1,
                        "3": [{"0": "MM3", "1": 500, "2": 1714949592200}]
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::NyseBook);
        let DataContent::NyseBook(items) = &payload.content else {
            panic!("expected NyseBook, got {:?}", payload.content);
        };
        let aapl = &items[0];
        assert_eq!(aapl.key, "AAPL");
        assert_eq!(aapl.market_snapshot_time, 1714949592300);

        assert_eq!(aapl.bid_side_levels.len(), 1);
        let bid = &aapl.bid_side_levels[0];
        assert_eq!(bid.price, dec!(150.00));
        assert_eq!(bid.aggregate_size, 1000);
        assert_eq!(bid.market_maker_count, 2);
        assert_eq!(bid.market_makers.len(), 2);
        assert_eq!(bid.market_makers[0].market_maker_id, "MM1");
        assert_eq!(bid.market_makers[0].size, 600);
        assert_eq!(bid.market_makers[0].quote_time, 1714949592000);

        assert_eq!(aapl.ask_side_levels.len(), 1);
        let ask = &aapl.ask_side_levels[0];
        assert_eq!(ask.price, dec!(150.05));
        assert_eq!(ask.aggregate_size, 500);
        assert_eq!(ask.market_maker_count, 1);
        assert_eq!(ask.market_makers[0].market_maker_id, "MM3");
    }

    #[test]
    fn parses_nasdaq_book_data_into_typed_content() {
        // Two bid levels (one with two MMs) and one ask level. Mirrors the
        // shape NYSE_BOOK uses since both share `book::Content`.
        let frame = r#"{
            "data": [{
                "service": "NASDAQ_BOOK",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "MSFT",
                    "delayed": false,
                    "1": 1714949592300,
                    "2": [
                        {
                            "0": 425.10,
                            "1": 800,
                            "2": 2,
                            "3": [
                                {"0": "MMA", "1": 500, "2": 1714949592000},
                                {"0": "MMB", "1": 300, "2": 1714949592100}
                            ]
                        },
                        {
                            "0": 425.05,
                            "1": 1200,
                            "2": 1,
                            "3": [{"0": "MMC", "1": 1200, "2": 1714949591900}]
                        }
                    ],
                    "3": [{
                        "0": 425.15,
                        "1": 600,
                        "2": 1,
                        "3": [{"0": "MMD", "1": 600, "2": 1714949592250}]
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::NasdaqBook);
        let DataContent::NasdaqBook(items) = &payload.content else {
            panic!("expected NasdaqBook, got {:?}", payload.content);
        };
        let msft = &items[0];
        assert_eq!(msft.key, "MSFT");
        assert_eq!(msft.market_snapshot_time, 1714949592300);

        assert_eq!(msft.bid_side_levels.len(), 2);
        assert_eq!(msft.bid_side_levels[0].price, dec!(425.10));
        assert_eq!(msft.bid_side_levels[0].aggregate_size, 800);
        assert_eq!(msft.bid_side_levels[0].market_makers.len(), 2);
        assert_eq!(msft.bid_side_levels[1].price, dec!(425.05));
        assert_eq!(msft.bid_side_levels[1].market_maker_count, 1);

        assert_eq!(msft.ask_side_levels.len(), 1);
        assert_eq!(msft.ask_side_levels[0].price, dec!(425.15));
        assert_eq!(msft.ask_side_levels[0].market_makers[0].market_maker_id, "MMD");
    }

    #[test]
    fn parses_options_book_data_into_typed_content() {
        // Same shape as NYSE/NASDAQ book; instrument key is a Schwab option
        // symbol. One bid level, one ask level, single MM each.
        let frame = r#"{
            "data": [{
                "service": "OPTIONS_BOOK",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL  240315C00200000",
                    "delayed": false,
                    "1": 1714949592300,
                    "2": [{
                        "0": 5.10,
                        "1": 12,
                        "2": 1,
                        "3": [{"0": "MMX", "1": 12, "2": 1714949592000}]
                    }],
                    "3": [{
                        "0": 5.20,
                        "1": 8,
                        "2": 1,
                        "3": [{"0": "MMY", "1": 8, "2": 1714949592200}]
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::OptionsBook);
        let DataContent::OptionsBook(items) = &payload.content else {
            panic!("expected OptionsBook, got {:?}", payload.content);
        };
        let opt = &items[0];
        assert_eq!(opt.key, "AAPL  240315C00200000");
        assert_eq!(opt.market_snapshot_time, 1714949592300);

        assert_eq!(opt.bid_side_levels.len(), 1);
        assert_eq!(opt.bid_side_levels[0].price, dec!(5.10));
        assert_eq!(opt.bid_side_levels[0].aggregate_size, 12);
        assert_eq!(opt.bid_side_levels[0].market_makers[0].market_maker_id, "MMX");

        assert_eq!(opt.ask_side_levels.len(), 1);
        assert_eq!(opt.ask_side_levels[0].price, dec!(5.20));
        assert_eq!(opt.ask_side_levels[0].market_makers[0].market_maker_id, "MMY");
    }

    #[test]
    fn parses_chart_equity_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "CHART_EQUITY",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL",
                    "delayed": false,
                    "1": 183.50, "2": 183.80, "3": 183.45, "4": 183.75,
                    "5": 125000,
                    "6": 1234,
                    "7": 1714949580000,
                    "8": 19850
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ChartEquity);
        let DataContent::ChartEquity(items) = &payload.content else {
            panic!("expected ChartEquity, got {:?}", payload.content);
        };
        let candle = &items[0];
        assert_eq!(candle.key, "AAPL");
        assert_eq!(candle.open_price, Some(dec!(183.50)));
        assert_eq!(candle.high_price, Some(dec!(183.80)));
        assert_eq!(candle.low_price, Some(dec!(183.45)));
        assert_eq!(candle.close_price, Some(dec!(183.75)));
        assert_eq!(candle.volume, Some(dec!(125000)));
        assert_eq!(candle.sequence, Some(1234));
        assert_eq!(candle.chart_time, Some(1714949580000));
        assert_eq!(candle.chart_day, Some(19850));
    }

    #[test]
    fn parses_chart_futures_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "CHART_FUTURES",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "/ESZ24",
                    "delayed": false,
                    "1": 1714949580000,
                    "2": 5020.00, "3": 5025.50, "4": 5018.25, "5": 5024.75,
                    "6": 8520
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ChartFutures);
        let DataContent::ChartFutures(items) = &payload.content else {
            panic!("expected ChartFutures, got {:?}", payload.content);
        };
        let candle = &items[0];
        assert_eq!(candle.key, "/ESZ24");
        assert_eq!(candle.chart_time, Some(1714949580000));
        assert_eq!(candle.open_price, Some(dec!(5020.00)));
        assert_eq!(candle.high_price, Some(dec!(5025.50)));
        assert_eq!(candle.low_price, Some(dec!(5018.25)));
        assert_eq!(candle.close_price, Some(dec!(5024.75)));
        assert_eq!(candle.volume, Some(dec!(8520)));
    }

    #[test]
    fn parses_screener_equity_data_into_typed_content() {
        // Two-item ranking on NYSE volume, 5-minute window. Items carry
        // camelCase named fields per Schwab's spec.
        let frame = r#"{
            "data": [{
                "service": "SCREENER_EQUITY",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "NYSE_VOLUME_5",
                    "delayed": false,
                    "1": 1714949590000,
                    "2": "VOLUME",
                    "3": 5,
                    "4": [
                        {
                            "description": "Apple Inc.",
                            "lastPrice": 183.50,
                            "marketShare": 1.25,
                            "netChange": 0.75,
                            "netPercentChange": 0.4106,
                            "symbol": "AAPL",
                            "totalVolume": 163224109,
                            "trades": 95012,
                            "volume": 12500000
                        },
                        {
                            "description": "Microsoft Corp.",
                            "lastPrice": 425.10,
                            "marketShare": 0.85,
                            "netChange": -1.20,
                            "netPercentChange": -0.2814,
                            "symbol": "MSFT",
                            "totalVolume": 22500000,
                            "trades": 41200,
                            "volume": 7250000
                        }
                    ]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ScreenerEquity);
        let DataContent::ScreenerEquity(rows) = &payload.content else {
            panic!("expected ScreenerEquity, got {:?}", payload.content);
        };
        let row = &rows[0];
        assert_eq!(row.key, "NYSE_VOLUME_5");
        assert_eq!(row.timestamp, Some(1714949590000));
        assert_eq!(row.sort_field.as_deref(), Some("VOLUME"));
        assert_eq!(row.frequency, Some(5));
        assert_eq!(row.items.len(), 2);

        let aapl = &row.items[0];
        assert_eq!(aapl.symbol.as_deref(), Some("AAPL"));
        assert_eq!(aapl.description.as_deref(), Some("Apple Inc."));
        assert_eq!(aapl.last_price, Some(dec!(183.50)));
        assert_eq!(aapl.market_share, Some(dec!(1.25)));
        assert_eq!(aapl.net_change, Some(dec!(0.75)));
        assert_eq!(aapl.net_percent_change, Some(dec!(0.4106)));
        assert_eq!(aapl.total_volume, Some(163224109));
        assert_eq!(aapl.trades, Some(95012));
        assert_eq!(aapl.volume, Some(12500000));

        let msft = &row.items[1];
        assert_eq!(msft.symbol.as_deref(), Some("MSFT"));
        assert_eq!(msft.net_change, Some(dec!(-1.20)));
    }

    #[test]
    fn parses_screener_option_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "SCREENER_OPTION",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "OPTION_CALL_VOLUME_5",
                    "delayed": false,
                    "1": 1714949590000,
                    "2": "VOLUME",
                    "3": 5,
                    "4": [{
                        "description": "AAPL Mar 15 2024 200 Call",
                        "lastPrice": 5.15,
                        "marketShare": 0.40,
                        "netChange": 0.05,
                        "netPercentChange": 0.9804,
                        "symbol": "AAPL  240315C00200000",
                        "totalVolume": 12345,
                        "trades": 312,
                        "volume": 8400
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ScreenerOption);
        let DataContent::ScreenerOption(rows) = &payload.content else {
            panic!("expected ScreenerOption, got {:?}", payload.content);
        };
        let row = &rows[0];
        assert_eq!(row.key, "OPTION_CALL_VOLUME_5");
        assert_eq!(row.sort_field.as_deref(), Some("VOLUME"));
        assert_eq!(row.frequency, Some(5));
        assert_eq!(row.items.len(), 1);

        let item = &row.items[0];
        assert_eq!(item.symbol.as_deref(), Some("AAPL  240315C00200000"));
        assert_eq!(item.last_price, Some(dec!(5.15)));
        assert_eq!(item.net_change, Some(dec!(0.05)));
        assert_eq!(item.volume, Some(8400));
    }

    #[test]
    fn parses_account_activity_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "ACCT_ACTIVITY",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "seq": 42,
                    "key": "my-correl-id",
                    "delayed": false,
                    "0": "my-correl-id",
                    "1": "12345678",
                    "2": "OrderEntryRequest",
                    "3": "{\"orderId\":\"ABC\",\"symbol\":\"AAPL\",\"quantity\":10}"
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::AccountActivity);
        let DataContent::AccountActivity(items) = &payload.content else {
            panic!("expected AccountActivity, got {:?}", payload.content);
        };
        let msg = &items[0];
        assert_eq!(msg.key, "my-correl-id");
        assert_eq!(msg.seq, Some(42));
        assert_eq!(msg.subscription_key.as_deref(), Some("my-correl-id"));
        assert_eq!(
            msg.account.as_ref().map(|a| a.expose_secret().to_string()),
            Some("12345678".to_string())
        );
        assert_eq!(msg.message_type.as_deref(), Some("OrderEntryRequest"));
        assert!(
            msg.message_data
                .as_deref()
                .map(|s| s.contains("AAPL"))
                .unwrap_or(false),
            "message_data should preserve raw payload"
        );
    }

    #[test]
    fn account_in_account_activity_redacts_on_debug() {
        // Compile-time check that Account is the redacted newtype.
        let frame = r#"{
            "data": [{
                "service": "ACCT_ACTIVITY",
                "timestamp": 1,
                "command": "SUBS",
                "content": [{
                    "seq": 1, "key": "k", "delayed": false,
                    "1": "12345678"
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let DataContent::AccountActivity(items) = &data[0].content else {
            panic!("expected AccountActivity");
        };
        let debug = format!("{:?}", items[0]);
        assert!(
            !debug.contains("12345678"),
            "account number leaked through Debug: {debug}"
        );
    }

    #[test]
    fn unknown_service_string_is_a_decode_error() {
        // Every documented Schwab service has a typed dispatcher arm and a
        // matching `Service` variant. A service string Schwab adds later
        // currently fails at the `Service` enum boundary. The
        // `DataContent::Raw` variant is reserved for the day `Service` grows
        // an `Unknown(String)` fallback.
        let frame = r#"{
            "data": [{
                "service": "BOND_BOOK",
                "timestamp": 1,
                "command": "SUBS",
                "content": [{"key":"AAA","1":1}]
            }]
        }"#;
        match parse(frame) {
            Err(Error::Decode { .. }) => {}
            other => panic!("expected Decode error, got {other:?}"),
        }
    }

    #[test]
    fn malformed_json_returns_decode_error() {
        let result = parse("not json at all");
        match result {
            Err(Error::Decode { .. }) => {}
            other => panic!("expected Decode error, got {other:?}"),
        }
    }

    #[test]
    fn malformed_data_content_returns_decode_error() {
        // `content` is supposed to be an array; passing a number triggers
        // the array-expected branch in `transform_keys`.
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_EQUITIES",
                "timestamp": 1,
                "command": "SUBS",
                "content": 42
            }]
        }"#;
        match parse(frame) {
            Err(Error::Decode { context, .. }) => {
                assert!(context.contains("content"), "context = {context}");
            }
            other => panic!("expected Decode error, got {other:?}"),
        }
    }
}
