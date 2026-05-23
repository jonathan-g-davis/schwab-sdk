use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fastwebsockets::{FragmentCollectorRead, WebSocketWrite};
use tokio::sync::{Mutex, watch};

use crate::error::{Error, Result};
use crate::secrets::{AuthToken, CustomerId};
use crate::streamer::events::{ConnectionEvent, DisconnectReason};
use crate::streamer::protocol::{ResponseCode, Service, StreamerCommand};
use crate::streamer::request::{RequestPayload, StreamerRequest};
use crate::streamer::response::{RawStreamerResponse, StreamerResponse};
use crate::streamer::subscription::SubscribeRequest;
use crate::streamer::{account_activity, admin, book, chart, level_one, screener};
use crate::websocket::WebSocket;

type Upgraded = hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>;
type WsReadHalf = FragmentCollectorRead<tokio::io::ReadHalf<Upgraded>>;
type WsWriteHalf = WebSocketWrite<tokio::io::WriteHalf<Upgraded>>;

/// Split a connected [`WebSocket`] into the [`ReadHalf`] and [`WriteHalf`]
/// the streamer surface exposes.
///
/// The websocket's write half is owned by an `Arc<Mutex<_>>` shared by both
/// halves: the writer locks it for `login`/`logout`/`send`, the reader locks
/// it inside `read_frame`'s control-frame callback to reply to pings and
/// close frames. No background task is spawned; all I/O happens inline on
/// the caller's own stack inside `recv()` / `send()`.
pub(crate) fn split(
    websocket: WebSocket,
    customer_id: CustomerId,
    correlation_id: String,
    channel: String,
    function_id: String,
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
        customer_id,
        correlation_id,
        channel,
        function_id,
        request_id: Arc::new(AtomicU64::new(0)),
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

pub struct ReadHalf {
    read_half: WsReadHalf,
    write_half: Arc<Mutex<WsWriteHalf>>,
    events_tx: watch::Sender<ConnectionEvent>,
}

impl ReadHalf {
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
                        return Err(Error::Decode {
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

#[derive(Clone)]
pub struct WriteHalf {
    write_half: Arc<Mutex<WsWriteHalf>>,
    customer_id: CustomerId,
    correlation_id: String,
    channel: String,
    function_id: String,
    request_id: Arc<AtomicU64>,
}

impl WriteHalf {
    /// Send the streamer LOGIN frame establishing the session. Must be
    /// called before any subscribe / add / unsubscribe / view request.
    /// Returns when the frame has been handed to the socket; the LOGIN
    /// ack arrives later on the read half as a `response` frame.
    pub async fn login(&self, auth_token: AuthToken) -> Result<()> {
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

        let serialized = serde_json::to_string(&request).map_err(|e| Error::Encode {
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
}
