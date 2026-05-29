use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fastwebsockets::{FragmentCollectorRead, WebSocketWrite};
use http::{
    Method,
    header::{CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
};
use http_body_util::Empty;
use hyper::{Request, Uri, body::Bytes};
use hyper_util::rt::TokioIo;
use rustls_platform_verifier::ConfigVerifierExt;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, watch};
use tokio_rustls::{TlsConnector, client::TlsStream, rustls};

use crate::error::{Error, Result};
use crate::secrets::CustomerId;
use crate::streamer::events::{ConnectionEvent, DisconnectReason};
use crate::streamer::protocol::{ResponseCode, Service, StreamerCommand};
use crate::streamer::request::{RequestPayload, StreamerRequest};
use crate::streamer::response::{RawStreamerResponse, StreamerResponse};
use crate::streamer::subscription::SubscribeRequest;
use crate::streamer::{account_activity, admin, book, chart, level_one, screener};
use crate::token::TokenProvider;
use crate::user_preferences::StreamerInfo;

type Upgraded = TokioIo<hyper::upgrade::Upgraded>;
type WsReadHalf = FragmentCollectorRead<tokio::io::ReadHalf<Upgraded>>;
type WsWriteHalf = WebSocketWrite<tokio::io::WriteHalf<Upgraded>>;
type WebSocket = fastwebsockets::WebSocket<Upgraded>;

/// Errors that surface from the streamer transport (TCP / TLS / WebSocket
/// handshake plus any frame-level error after the socket is up).
#[derive(Debug, thiserror::Error)]
pub enum WebSocketError {
    /// TCP connect failed.
    #[error("failed to connect to server")]
    Connect(std::io::Error),
    /// WebSocket upgrade handshake failed.
    #[error("failed to perform websocket handshake")]
    Handshake(fastwebsockets::WebSocketError),
    /// `streamerSocketUrl` host is not a valid DNS name.
    #[error("invalid domain")]
    InvalidDomain(rustls_pki_types::InvalidDnsNameError),
    /// `streamerSocketUrl` did not include a host component.
    #[error("host is required")]
    MissingHost,
    /// TLS handshake failed on top of the TCP socket.
    #[error("failed to create TLS stream")]
    TlsStream(std::io::Error),
    /// Building the rustls client config failed.
    #[error("failed to configure TLS: {0}")]
    TlsConfig(rustls::Error),
    /// Building the HTTP upgrade request failed.
    #[error("failed to build upgrade request: {0}")]
    BuildRequest(http::Error),
    /// `streamerSocketUrl` used a scheme that is not permitted for the
    /// current build. `wss://` is always accepted; `ws://` is accepted
    /// only in debug builds, because a plaintext WebSocket would carry
    /// the bearer token in the LOGIN frame in the clear. Any other
    /// scheme (or a URL with no scheme at all) is always rejected.
    #[error("unsupported websocket scheme: {0}")]
    UnsupportedScheme(String),
    /// Runtime frame error after the websocket is up: read/write/control
    /// frame failures from `fastwebsockets`.
    #[error("websocket runtime error: {0}")]
    Runtime(#[from] fastwebsockets::WebSocketError),
}

impl WebSocketError {
    /// Whether a fresh `connect` (and re-login) is worth attempting after
    /// this error. Returns `false` for configuration-shaped failures that
    /// will fail identically on retry (bad scheme, missing host, malformed
    /// upgrade request, rustls config error) and `true` for transport- or
    /// session-level failures (TCP connect, TLS handshake, WebSocket
    /// handshake, post-handshake frame errors).
    ///
    /// Used by [`crate::Error::is_retryable`] to classify
    /// [`crate::Error::WebSocket`].
    pub fn is_retryable(&self) -> bool {
        match self {
            WebSocketError::Connect(_)
            | WebSocketError::TlsStream(_)
            | WebSocketError::Handshake(_)
            | WebSocketError::Runtime(_) => true,
            WebSocketError::InvalidDomain(_)
            | WebSocketError::MissingHost
            | WebSocketError::TlsConfig(_)
            | WebSocketError::BuildRequest(_)
            | WebSocketError::UnsupportedScheme(_) => false,
        }
    }
}

impl From<fastwebsockets::WebSocketError> for Error {
    fn from(value: fastwebsockets::WebSocketError) -> Self {
        Error::WebSocket(WebSocketError::Runtime(value))
    }
}

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

async fn connect_tls(uri: &Uri) -> std::result::Result<TlsStream<TcpStream>, WebSocketError> {
    let host = uri.host().ok_or(WebSocketError::MissingHost)?;
    let port = uri.port_u16().unwrap_or(443);
    let addr = format!("{}:{}", host, port);

    let socket = TcpStream::connect(addr)
        .await
        .map_err(WebSocketError::Connect)?;

    let domain = rustls_pki_types::ServerName::try_from(host.to_string())
        .map_err(WebSocketError::InvalidDomain)?;
    let config =
        rustls::ClientConfig::with_platform_verifier().map_err(WebSocketError::TlsConfig)?;
    let connector = TlsConnector::from(Arc::new(config));
    connector
        .connect(domain, socket)
        .await
        .map_err(WebSocketError::TlsStream)
}

async fn connect_tcp(uri: &Uri) -> std::result::Result<TcpStream, WebSocketError> {
    let host = uri.host().ok_or(WebSocketError::MissingHost)?;
    let port = uri.port_u16().unwrap_or(80);
    TcpStream::connect(format!("{}:{}", host, port))
        .await
        .map_err(WebSocketError::Connect)
}

/// Which transport to use for a given streamer URL scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WsTransport {
    /// TLS handshake on top of TCP (`wss://`).
    Tls,
    /// Plain TCP (`ws://`). Reachable only in debug builds; the
    /// streamer LOGIN frame would otherwise put the bearer on the wire
    /// in cleartext.
    Plain,
}

/// Map a URI scheme to the [`WsTransport`] to use. `allow_insecure`
/// gates `ws://`; release builds set it to `false`.
///
/// Extracted from [`connect_websocket`] so both modes are unit-testable
/// from a single test binary without rebuilding in release mode.
fn check_websocket_scheme(
    scheme: Option<&str>,
    allow_insecure: bool,
) -> std::result::Result<WsTransport, WebSocketError> {
    match scheme {
        Some("wss") => Ok(WsTransport::Tls),
        Some("ws") if allow_insecure => Ok(WsTransport::Plain),
        Some("ws") => Err(WebSocketError::UnsupportedScheme("ws".to_string())),
        Some(other) => Err(WebSocketError::UnsupportedScheme(other.to_string())),
        None => Err(WebSocketError::UnsupportedScheme(String::new())),
    }
}

async fn connect_websocket(uri: &Uri) -> std::result::Result<WebSocket, WebSocketError> {
    let transport = check_websocket_scheme(uri.scheme_str(), cfg!(debug_assertions))?;

    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(HOST, uri.host().ok_or(WebSocketError::MissingHost)?)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(SEC_WEBSOCKET_KEY, fastwebsockets::handshake::generate_key())
        .header(SEC_WEBSOCKET_VERSION, "13")
        .body(Empty::<Bytes>::new())
        .map_err(WebSocketError::BuildRequest)?;

    match transport {
        WsTransport::Tls => {
            let stream = connect_tls(uri).await?;
            let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
                .await
                .map_err(WebSocketError::Handshake)?;
            Ok(ws)
        }
        WsTransport::Plain => {
            let stream = connect_tcp(uri).await?;
            let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
                .await
                .map_err(WebSocketError::Handshake)?;
            Ok(ws)
        }
    }
}

/// Open the streamer websocket using the connection details from
/// `/userPreference` and return the read and write halves of the session.
/// Call [`WriteHalf::login`] before any other command.
///
/// `token_provider` is the [`TokenProvider`] used to fetch the bearer for
/// the LOGIN frame. It is consulted at LOGIN-frame construction so a token
/// rotated in the provider after `connect` returns is the one carried on
/// the wire when `login` is called.
///
/// Every field on `streamer_info` is `Option` per the spec; this function
/// validates that the fields needed to log in and route subscribe frames
/// (socket URL, customer id, correlation id, channel, function id) are
/// all present, returning [`Error::InvalidPreference`] for the first
/// missing one.
pub async fn connect(
    streamer_info: StreamerInfo,
    token_provider: Arc<dyn TokenProvider + Send + Sync>,
) -> Result<(ReadHalf, WriteHalf)> {
    let validated = ValidatedStreamerInfo::try_from(streamer_info)?;
    let websocket = connect_websocket(&validated.socket_url).await?;
    Ok(split(websocket, validated, token_provider))
}

/// `StreamerInfo` after the per-field optionality has been resolved.
/// Constructing one of these is the only way to reach [`split`].
#[derive(Debug)]
struct ValidatedStreamerInfo {
    socket_url: Uri,
    customer_id: CustomerId,
    correlation_id: String,
    channel: String,
    function_id: String,
}

impl TryFrom<StreamerInfo> for ValidatedStreamerInfo {
    type Error = Error;

    fn try_from(info: StreamerInfo) -> Result<Self> {
        fn required<T>(field: &'static str, value: Option<T>) -> Result<T> {
            value.ok_or(Error::InvalidPreference {
                field,
                reason: "missing".to_string(),
            })
        }

        let socket_url = required("streamerSocketUrl", info.streamer_socket_url)?
            .parse::<Uri>()
            .map_err(|e| Error::InvalidPreference {
                field: "streamerSocketUrl",
                reason: e.to_string(),
            })?;

        Ok(Self {
            socket_url,
            customer_id: required("schwabClientCustomerId", info.schwab_client_customer_id)?,
            correlation_id: required("schwabClientCorrelId", info.schwab_client_correlation_id)?,
            channel: required("schwabClientChannel", info.schwab_client_channel)?,
            function_id: required("schwabClientFunctionId", info.schwab_client_function_id)?,
        })
    }
}

/// Split a connected [`WebSocket`] into the [`ReadHalf`] and [`WriteHalf`]
/// the streamer surface exposes.
///
/// The websocket's write half is owned by an `Arc<Mutex<_>>` shared by both
/// halves: the writer locks it for `login`/`logout`/`send`, the reader locks
/// it inside `read_frame`'s control-frame callback to reply to pings and
/// close frames. No background task is spawned; all I/O happens inline on
/// the caller's own stack inside `recv()` / `send()`.
fn split(
    websocket: WebSocket,
    streamer_info: ValidatedStreamerInfo,
    token_provider: Arc<dyn TokenProvider + Send + Sync>,
) -> (ReadHalf, WriteHalf) {
    let (read_half, write_half) = websocket.split(tokio::io::split);
    let write_half = Arc::new(Mutex::new(write_half));
    let (events_tx, _) = watch::channel(ConnectionEvent::Connected);

    let reader = ReadHalf {
        read_half: FragmentCollectorRead::new(read_half),
        write_half: write_half.clone(),
        events_tx,
    };

    let writer = WriteHalf {
        write_half,
        customer_id: streamer_info.customer_id,
        correlation_id: streamer_info.correlation_id,
        channel: streamer_info.channel,
        function_id: streamer_info.function_id,
        request_id: Arc::new(AtomicU64::new(0)),
        token_provider,
    };

    (reader, writer)
}

/// Lock the shared write half and write a single frame. Used both by the
/// reader (to reply to ping/close control frames) and the writer (to send
/// requests). Lifting this out of the closure that `read_frame` consumes
/// makes the future's lifetime relation to `frame` explicit, which the
/// closure form (with an `async move` block) cannot express on stable Rust.
async fn write_one(
    write_half: Arc<Mutex<WsWriteHalf>>,
    frame: fastwebsockets::Frame<'_>,
) -> std::result::Result<(), fastwebsockets::WebSocketError> {
    write_half.lock().await.write_frame(frame).await
}

/// Read half of the streamer session. Yields one
/// [`StreamerResponse`] per [`Self::recv`] call. Cloneable through
/// [`Self::events`] for connection-state observation only; the read half
/// itself is single-consumer.
pub struct ReadHalf {
    read_half: WsReadHalf,
    write_half: Arc<Mutex<WsWriteHalf>>,
    events_tx: watch::Sender<ConnectionEvent>,
}

impl std::fmt::Debug for ReadHalf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadHalf").finish_non_exhaustive()
    }
}

impl ReadHalf {
    /// Receive the next streamer frame.
    ///
    /// Blocks until a text frame arrives, then parses it into a
    /// [`StreamerResponse`]. Control frames (ping/pong/close) are handled
    /// inline, so this method only returns on real protocol traffic.
    ///
    /// Errors:
    /// - [`Error::WebSocket`](crate::Error::WebSocket) on transport
    ///   failure (the [`ConnectionEvent::Disconnected`] event also fires
    ///   on the watch channel returned by [`Self::events`]).
    /// - [`Error::Codec`](crate::Error::Codec) on a malformed frame.
    pub async fn recv(&mut self) -> Result<StreamerResponse> {
        let write_half = self.write_half.clone();
        let mut send_fn = move |frame| write_one(write_half.clone(), frame);
        loop {
            let frame = match self.read_half.read_frame(&mut send_fn).await {
                Ok(f) => f,
                Err(e) => {
                    self.events_tx.send_replace(ConnectionEvent::Disconnected(
                        DisconnectReason::Transport(e.to_string()),
                    ));
                    return Err(e.into());
                }
            };
            if frame.opcode == fastwebsockets::OpCode::Text {
                let raw_response: RawStreamerResponse = match serde_json::from_slice(&frame.payload)
                {
                    Ok(r) => r,
                    Err(e) => {
                        self.events_tx.send_replace(ConnectionEvent::StreamError {
                            message: e.to_string(),
                        });
                        return Err(Error::Codec {
                            context: "streamer response frame".to_string(),
                            reason: e.to_string(),
                        });
                    }
                };
                let response = StreamerResponse::try_from(raw_response)?;
                classify_and_emit(&self.events_tx, &response);
                return Ok(response);
            }
        }
    }

    /// Subscribe to connection-state updates for this session. Receivers
    /// initially observe the current state (typically `Connected` or, after
    /// the first login response, `LoggedIn`).
    ///
    /// # Examples
    ///
    /// Drive a reconnect decision off the state stream. The reconnect loop
    /// itself lives in consumer code; this side only surfaces the signal.
    ///
    /// ```no_run
    /// use schwab_sdk::streamer::{ConnectionEvent, ReadHalf};
    ///
    /// # async fn run(read: &ReadHalf) {
    /// let mut events = read.events();
    /// while events.changed().await.is_ok() {
    ///     match &*events.borrow_and_update() {
    ///         ConnectionEvent::LoggedIn => println!("session ready"),
    ///         ConnectionEvent::Disconnected(reason) => {
    ///             println!("disconnected: {reason:?}");
    ///             break;
    ///         }
    ///         other => println!("state: {other:?}"),
    ///     }
    /// }
    /// # }
    /// ```
    pub fn events(&self) -> watch::Receiver<ConnectionEvent> {
        self.events_tx.subscribe()
    }
}

/// Classify a parsed `StreamerResponse` and emit any state changes through
/// `events_tx`. Errors are not emitted here; the caller handles them.
fn classify_and_emit(events_tx: &watch::Sender<ConnectionEvent>, response: &StreamerResponse) {
    let StreamerResponse::Response(responses) = response else {
        return;
    };
    for r in responses {
        let is_login = r.service == Service::Admin && r.command == StreamerCommand::Login;
        match r.content.code {
            ResponseCode::Ok if is_login => {
                events_tx.send_replace(ConnectionEvent::LoggedIn);
            }
            ResponseCode::LoginDenied => {
                events_tx.send_replace(ConnectionEvent::Disconnected(
                    DisconnectReason::LoginDenied(r.content.message.clone()),
                ));
            }
            ResponseCode::CloseConnection => {
                events_tx.send_replace(ConnectionEvent::Disconnected(
                    DisconnectReason::ServerClose(r.content.message.clone()),
                ));
            }
            ResponseCode::StopStreaming => {
                events_tx.send_replace(ConnectionEvent::Disconnected(
                    DisconnectReason::StopStreaming(r.content.message.clone()),
                ));
            }
            _ => {}
        }
    }
}

/// Write half of the streamer session. Sends login/logout/subscribe
/// frames. Cloneable: all clones share the same underlying socket,
/// monotonic request-id counter, and [`TokenProvider`], so they can be
/// moved into independent tasks safely.
#[derive(Clone)]
pub struct WriteHalf {
    write_half: Arc<Mutex<WsWriteHalf>>,
    customer_id: CustomerId,
    correlation_id: String,
    channel: String,
    function_id: String,
    request_id: Arc<AtomicU64>,
    token_provider: Arc<dyn TokenProvider + Send + Sync>,
}

impl std::fmt::Debug for WriteHalf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteHalf")
            .field("channel", &self.channel)
            .field("function_id", &self.function_id)
            .finish_non_exhaustive()
    }
}

impl WriteHalf {
    /// Send the streamer LOGIN frame establishing the session. Must be
    /// called before any subscribe/add/unsubscribe/view request.
    /// Returns when the frame has been handed to the socket; the LOGIN
    /// ack arrives later on the read half as a `response` frame.
    ///
    /// The bearer carried by the frame is fetched from the
    /// [`TokenProvider`] supplied to [`connect`] at the moment `login`
    /// is called - calling `login` again after the provider observes a
    /// rotated token will re-LOGIN with the new value.
    /// [`Error::TokenProvider`] surfaces if the provider fails before
    /// any frame is written.
    pub async fn login(&self) -> Result<()> {
        let auth_token = self.token_provider.access_token().await?;
        let request = admin::Login {
            authorization: auth_token,
            schwab_client_channel: self.channel.clone(),
            schwab_client_function_id: self.function_id.clone(),
        };
        self.send(request).await
    }

    /// Send the streamer LOGOUT frame.
    pub async fn logout(&self) -> Result<()> {
        self.send(admin::Logout).await
    }

    /// LEVELONE_EQUITIES subscription entry point.
    pub fn equities(&self) -> SubscribeRequest<'_, level_one::equities::Field> {
        SubscribeRequest::new(self)
    }

    /// LEVELONE_OPTIONS subscription entry point.
    pub fn options(&self) -> SubscribeRequest<'_, level_one::options::Field> {
        SubscribeRequest::new(self)
    }

    /// LEVELONE_FUTURES subscription entry point.
    pub fn futures(&self) -> SubscribeRequest<'_, level_one::futures::Field> {
        SubscribeRequest::new(self)
    }

    /// LEVELONE_FUTURES_OPTIONS subscription entry point.
    pub fn futures_options(&self) -> SubscribeRequest<'_, level_one::futures_options::Field> {
        SubscribeRequest::new(self)
    }

    /// LEVELONE_FOREX subscription entry point.
    pub fn forex(&self) -> SubscribeRequest<'_, level_one::forex::Field> {
        SubscribeRequest::new(self)
    }

    /// NYSE_BOOK subscription entry point.
    pub fn nyse_book(&self) -> SubscribeRequest<'_, book::nyse::Field> {
        SubscribeRequest::new(self)
    }

    /// NASDAQ_BOOK subscription entry point.
    pub fn nasdaq_book(&self) -> SubscribeRequest<'_, book::nasdaq::Field> {
        SubscribeRequest::new(self)
    }

    /// OPTIONS_BOOK subscription entry point.
    pub fn options_book(&self) -> SubscribeRequest<'_, book::options::Field> {
        SubscribeRequest::new(self)
    }

    /// CHART_EQUITY subscription entry point.
    pub fn chart_equity(&self) -> SubscribeRequest<'_, chart::equity::Field> {
        SubscribeRequest::new(self)
    }

    /// CHART_FUTURES subscription entry point.
    pub fn chart_futures(&self) -> SubscribeRequest<'_, chart::futures::Field> {
        SubscribeRequest::new(self)
    }

    /// SCREENER_EQUITY subscription entry point.
    pub fn screener_equity(&self) -> SubscribeRequest<'_, screener::equity::Field> {
        SubscribeRequest::new(self)
    }

    /// SCREENER_OPTION subscription entry point.
    pub fn screener_option(&self) -> SubscribeRequest<'_, screener::option::Field> {
        SubscribeRequest::new(self)
    }

    /// ACCT_ACTIVITY subscription entry point.
    pub fn account_activity(&self) -> SubscribeRequest<'_, account_activity::Field> {
        SubscribeRequest::new(self)
    }

    /// Serialize a built [`StreamerRequest`] and write it as one frame.
    /// Crate-internal: external callers reach this only through the typed
    /// service accessors above (and through [`Self::login`] /
    /// [`Self::logout`]).
    pub(crate) async fn send<T: Into<StreamerRequest>>(&self, request: T) -> Result<()> {
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

        let serialized = serde_json::to_string(&request).map_err(|e| Error::Codec {
            context: "streamer request envelope".to_string(),
            reason: e.to_string(),
        })?;
        write_one(
            self.write_half.clone(),
            fastwebsockets::Frame::text(fastwebsockets::Payload::Borrowed(serialized.as_bytes())),
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::events::{ConnectionEvent, DisconnectReason};
    use crate::streamer::protocol::{ResponseCode, Service, StreamerCommand};
    use crate::streamer::response::{ResponseContent, ResponsePayload};

    fn response(code: ResponseCode, command: StreamerCommand, msg: &str) -> StreamerResponse {
        StreamerResponse::Response(vec![ResponsePayload {
            request_id: 1,
            service: Service::Admin,
            timestamp: 1,
            command,
            schwab_client_correlation_id: "x".into(),
            content: ResponseContent {
                code,
                message: msg.into(),
            },
        }])
    }

    fn full_streamer_info() -> StreamerInfo {
        StreamerInfo {
            streamer_socket_url: Some("wss://streamer-api.schwab.com/ws".into()),
            schwab_client_customer_id: Some(CustomerId::from("CUSTID")),
            schwab_client_correlation_id: Some("abc-123".into()),
            schwab_client_channel: Some("N9".into()),
            schwab_client_function_id: Some("APIAPP".into()),
        }
    }

    #[test]
    fn validates_complete_streamer_info() {
        let validated =
            ValidatedStreamerInfo::try_from(full_streamer_info()).expect("complete info validates");
        assert_eq!(validated.socket_url, "wss://streamer-api.schwab.com/ws");
        assert_eq!(validated.correlation_id, "abc-123");
        assert_eq!(validated.channel, "N9");
        assert_eq!(validated.function_id, "APIAPP");
    }

    #[test]
    fn missing_socket_url_reports_field() {
        let mut info = full_streamer_info();
        info.streamer_socket_url = None;
        match ValidatedStreamerInfo::try_from(info) {
            Err(Error::InvalidPreference { field, .. }) => {
                assert_eq!(field, "streamerSocketUrl");
            }
            other => panic!("expected InvalidPreference, got {other:?}"),
        }
    }

    #[test]
    fn missing_customer_id_reports_field() {
        let mut info = full_streamer_info();
        info.schwab_client_customer_id = None;
        match ValidatedStreamerInfo::try_from(info) {
            Err(Error::InvalidPreference { field, .. }) => {
                assert_eq!(field, "schwabClientCustomerId");
            }
            other => panic!("expected InvalidPreference, got {other:?}"),
        }
    }

    #[test]
    fn missing_correlation_id_reports_field() {
        let mut info = full_streamer_info();
        info.schwab_client_correlation_id = None;
        match ValidatedStreamerInfo::try_from(info) {
            Err(Error::InvalidPreference { field, .. }) => {
                assert_eq!(field, "schwabClientCorrelId");
            }
            other => panic!("expected InvalidPreference, got {other:?}"),
        }
    }

    #[test]
    fn missing_channel_reports_field() {
        let mut info = full_streamer_info();
        info.schwab_client_channel = None;
        match ValidatedStreamerInfo::try_from(info) {
            Err(Error::InvalidPreference { field, .. }) => {
                assert_eq!(field, "schwabClientChannel");
            }
            other => panic!("expected InvalidPreference, got {other:?}"),
        }
    }

    #[test]
    fn missing_function_id_reports_field() {
        let mut info = full_streamer_info();
        info.schwab_client_function_id = None;
        match ValidatedStreamerInfo::try_from(info) {
            Err(Error::InvalidPreference { field, .. }) => {
                assert_eq!(field, "schwabClientFunctionId");
            }
            other => panic!("expected InvalidPreference, got {other:?}"),
        }
    }

    #[test]
    fn login_ok_emits_logged_in() {
        let (tx, mut rx) = watch::channel(ConnectionEvent::Connected);
        classify_and_emit(&tx, &response(ResponseCode::Ok, StreamerCommand::Login, ""));
        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), ConnectionEvent::LoggedIn);
    }

    #[test]
    fn login_denied_emits_disconnected() {
        let (tx, mut rx) = watch::channel(ConnectionEvent::Connected);
        classify_and_emit(
            &tx,
            &response(
                ResponseCode::LoginDenied,
                StreamerCommand::Login,
                "token expired",
            ),
        );
        match rx.borrow_and_update().clone() {
            ConnectionEvent::Disconnected(DisconnectReason::LoginDenied(msg)) => {
                assert!(msg.contains("token expired"), "msg = {msg}");
            }
            other => panic!("expected Disconnected(LoginDenied), got {other:?}"),
        }
    }

    #[test]
    fn close_connection_emits_disconnected_server_close() {
        let (tx, mut rx) = watch::channel(ConnectionEvent::Connected);
        classify_and_emit(
            &tx,
            &response(
                ResponseCode::CloseConnection,
                StreamerCommand::Subs,
                "max connections",
            ),
        );
        assert!(matches!(
            *rx.borrow_and_update(),
            ConnectionEvent::Disconnected(DisconnectReason::ServerClose(_))
        ));
    }

    #[test]
    fn stop_streaming_emits_disconnected_stop_streaming() {
        let (tx, mut rx) = watch::channel(ConnectionEvent::Connected);
        classify_and_emit(
            &tx,
            &response(
                ResponseCode::StopStreaming,
                StreamerCommand::Subs,
                "inactivity",
            ),
        );
        assert!(matches!(
            *rx.borrow_and_update(),
            ConnectionEvent::Disconnected(DisconnectReason::StopStreaming(_))
        ));
    }

    #[test]
    fn non_admin_ok_response_does_not_emit() {
        let (tx, rx) = watch::channel(ConnectionEvent::Connected);
        // SUBS success on LEVELONE_EQUITIES should not flip to LoggedIn.
        let r = StreamerResponse::Response(vec![ResponsePayload {
            request_id: 1,
            service: Service::LevelOneEquities,
            timestamp: 1,
            command: StreamerCommand::Subs,
            schwab_client_correlation_id: "x".into(),
            content: ResponseContent {
                code: ResponseCode::Ok,
                message: "".into(),
            },
        }]);
        classify_and_emit(&tx, &r);
        // No change observed.
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn data_payload_does_not_emit() {
        let (tx, rx) = watch::channel(ConnectionEvent::Connected);
        let r = StreamerResponse::Notify(vec![]);
        classify_and_emit(&tx, &r);
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn wss_is_accepted_in_both_modes() {
        assert_eq!(
            check_websocket_scheme(Some("wss"), false).unwrap(),
            WsTransport::Tls
        );
        assert_eq!(
            check_websocket_scheme(Some("wss"), true).unwrap(),
            WsTransport::Tls
        );
    }

    #[test]
    fn ws_is_rejected_when_insecure_disallowed() {
        match check_websocket_scheme(Some("ws"), false) {
            Err(WebSocketError::UnsupportedScheme(scheme)) => assert_eq!(scheme, "ws"),
            other => panic!("expected UnsupportedScheme(ws), got {other:?}"),
        }
    }

    #[test]
    fn ws_is_accepted_when_insecure_permitted() {
        assert_eq!(
            check_websocket_scheme(Some("ws"), true).unwrap(),
            WsTransport::Plain
        );
    }

    #[test]
    fn other_schemes_are_always_rejected() {
        for scheme in ["http", "https", "ftp", "file", ""] {
            assert!(
                matches!(
                    check_websocket_scheme(Some(scheme), true).unwrap_err(),
                    WebSocketError::UnsupportedScheme(_)
                ),
                "scheme {scheme:?} should be rejected with insecure mode on"
            );
            assert!(
                matches!(
                    check_websocket_scheme(Some(scheme), false).unwrap_err(),
                    WebSocketError::UnsupportedScheme(_)
                ),
                "scheme {scheme:?} should be rejected with insecure mode off"
            );
        }
    }

    #[test]
    fn no_scheme_is_rejected() {
        assert!(matches!(
            check_websocket_scheme(None, true).unwrap_err(),
            WebSocketError::UnsupportedScheme(s) if s.is_empty()
        ));
        assert!(matches!(
            check_websocket_scheme(None, false).unwrap_err(),
            WebSocketError::UnsupportedScheme(s) if s.is_empty()
        ));
    }

    #[test]
    fn case_sensitive_scheme_match() {
        assert!(check_websocket_scheme(Some("Wss"), false).is_err(),);
        assert!(check_websocket_scheme(Some("WSS"), false).is_err(),);
    }

    #[test]
    fn is_retryable_classifies_transport_failures_as_retryable() {
        // TCP / TLS / handshake / runtime errors all warrant a reconnect.
        assert!(WebSocketError::Connect(std::io::Error::other("x")).is_retryable());
        assert!(WebSocketError::TlsStream(std::io::Error::other("x")).is_retryable());
        assert!(
            WebSocketError::Handshake(fastwebsockets::WebSocketError::ConnectionClosed)
                .is_retryable()
        );
        assert!(
            WebSocketError::Runtime(fastwebsockets::WebSocketError::ConnectionClosed)
                .is_retryable()
        );
    }

    #[test]
    fn is_retryable_classifies_config_failures_as_terminal() {
        // These will fail identically on retry; callers must not loop.
        assert!(!WebSocketError::MissingHost.is_retryable());
        assert!(!WebSocketError::UnsupportedScheme("ws".to_string()).is_retryable());
        assert!(
            !WebSocketError::InvalidDomain(
                rustls_pki_types::ServerName::try_from("not a dns name").unwrap_err()
            )
            .is_retryable()
        );
        // `BuildRequest` and `TlsConfig` carry foreign error types that
        // are awkward to fabricate in a unit test; the exhaustive match
        // in `is_retryable` keeps them classified alongside the others
        // here, and the surrounding `match` would fail to compile if a
        // new variant were added without an explicit decision.
    }

    #[test]
    fn error_is_retryable_delegates_to_websocket_error() {
        // The parent `Error::is_retryable` used to blanket-return `true`
        // for every `Error::WebSocket`; verify the per-variant path now
        // surfaces a terminal config error as terminal.
        let terminal = Error::WebSocket(WebSocketError::UnsupportedScheme("ws".to_string()));
        assert!(!terminal.is_retryable());
        let transient = Error::WebSocket(WebSocketError::Connect(std::io::Error::other(
            "conn refused",
        )));
        assert!(transient.is_retryable());
    }
}
