//! Connection-state events for the streamer.
//!
//! The streamer publishes a `watch::Receiver<ConnectionEvent>` so consumers
//! can observe the lifecycle of a single WebSocket session: connect, login
//! outcome, transient stream errors, and disconnect. The channel is meant
//! for fan-out to other tasks (UI, monitoring); the consumer's own
//! reconnect loop typically also learns about disconnects from `recv()`
//! returning `Err`.
//!
//! `Connected` is the watch channel's initial value: the streamer only
//! exists once the WS handshake has succeeded.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// WS handshake complete; no login attempt observed yet on this session.
    Connected,
    /// Login response was received with `code = Ok`.
    LoggedIn,
    /// Recoverable error on the stream. The connection may still be alive;
    /// the consumer can choose to keep reading or treat as fatal.
    StreamError { message: String },
    /// The connection is dead and the consumer should reconnect.
    Disconnected(DisconnectReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
    /// WebSocket transport-level failure (read_frame error, EOF, etc.).
    Transport(String),
    /// Schwab denied the login (response code `LOGIN_DENIED`).
    LoginDenied(String),
    /// Schwab issued `CLOSE_CONNECTION`.
    ServerClose(String),
    /// Schwab issued `STOP_STREAMING`.
    StopStreaming(String),
}
