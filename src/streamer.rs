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

pub mod admin;
pub mod level_one;
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
    fn unknown_service_falls_back_to_raw() {
        let frame = r#"{
            "data": [{
                "service": "CHART_EQUITY",
                "timestamp": 1,
                "command": "SUBS",
                "content": [{"key":"AAPL","1":1,"2":2,"3":3,"4":4}]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        match &data[0].content {
            DataContent::Raw(v) => {
                assert!(v.is_array(), "expected raw array, got {v:?}");
            }
            other => panic!("expected Raw fallback, got {other:?}"),
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
