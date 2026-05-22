use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use derive_builder::Builder;
use fastwebsockets::{FragmentCollectorRead, WebSocketWrite};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::model::{AuthToken, CustomerId};
use crate::streamer::events::{ConnectionEvent, DisconnectReason};
use crate::streamer::protocol::{Command, ResponseCode, Service};
use crate::streamer::request::{RequestPayload, StreamerRequest};
use crate::streamer::response::{RawStreamerResponse, StreamerResponse};
use crate::websocket::WebSocket;

type Upgraded = hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>;
type ReadHalf = FragmentCollectorRead<tokio::io::ReadHalf<Upgraded>>;
type WriteHalf = WebSocketWrite<tokio::io::WriteHalf<Upgraded>>;

pub struct SchwabStreamerReadHalf {
    read_half: ReadHalf,
    sender: mpsc::Sender<fastwebsockets::Frame<'static>>,
    events_tx: watch::Sender<ConnectionEvent>,
}

impl SchwabStreamerReadHalf {
    pub async fn recv(&mut self) -> Result<StreamerResponse> {
        let mut send_fn = Box::new(|frame| self.sender.send(frame));
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
        let is_login = r.service == Service::Admin && r.command == Command::Login;
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
            .send(fastwebsockets::Frame::text(fastwebsockets::Payload::Owned(
                serialized.into(),
            )))
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
    #[builder(default = "default_events_sender()")]
    events_tx: watch::Sender<ConnectionEvent>,
}

/// Initial event-channel sender. The receiver returned by `watch::channel`
/// is intentionally dropped: the sender retains the initial `Connected`
/// value and `send_replace` continues to update it even with no live
/// receivers, so consumers who later call `events()` see the most-recent
/// state.
fn default_events_sender() -> watch::Sender<ConnectionEvent> {
    watch::channel(ConnectionEvent::Connected).0
}

impl SchwabStreamer {
    pub(crate) fn builder() -> SchwabStreamerBuilder {
        SchwabStreamerBuilder::default()
    }

    /// Subscribe to connection-state updates for this session.
    pub fn events(&self) -> watch::Receiver<ConnectionEvent> {
        self.events_tx.subscribe()
    }

    pub fn split(self) -> (SchwabStreamerReadHalf, SchwabStreamerWriteHalf, FrameSender) {
        let (tx, rx) = mpsc::channel::<fastwebsockets::Frame<'static>>(100);
        let (read_half, write_half) = self.websocket.split(tokio::io::split);

        let reader = SchwabStreamerReadHalf {
            read_half: FragmentCollectorRead::new(read_half),
            sender: tx.clone(),
            events_tx: self.events_tx,
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
            let frame = match self.websocket.read_frame().await {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::events::{ConnectionEvent, DisconnectReason};
    use crate::streamer::protocol::{Command, ResponseCode, Service};
    use crate::streamer::response::{ResponseContent, ResponsePayload};

    fn response(code: ResponseCode, command: Command, msg: &str) -> StreamerResponse {
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
        classify_and_emit(&tx, &response(ResponseCode::Ok, Command::Login, ""));
        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), ConnectionEvent::LoggedIn);
    }

    #[test]
    fn login_denied_emits_disconnected() {
        let (tx, mut rx) = watch::channel(ConnectionEvent::Connected);
        classify_and_emit(
            &tx,
            &response(ResponseCode::LoginDenied, Command::Login, "token expired"),
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
                Command::Subs,
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
            &response(ResponseCode::StopStreaming, Command::Subs, "inactivity"),
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
            command: Command::Subs,
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
