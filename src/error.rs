//! Crate-wide error type.
//!
//! Every fallible operation in `schwab-rs` returns [`Result<T>`] aliasing
//! `std::result::Result<T, Error>`. Variants are kept structured wherever
//! Schwab gives us enough information; `Encode`/`Decode`/`Build` carry a
//! `context` string for when something goes wrong.
//!
//! [`Error::is_retryable`] and [`Error::retry_after`] are the only retry
//! seams the crate provides - application code can use these to wire in
//! `backon` or another policy on top.

use http::StatusCode;

use crate::websocket;

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
    #[error("encode {context}: {reason}")]
    Encode { context: String, reason: String },
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
    /// Schwab returned 201 from a place / replace order endpoint without
    /// a `Location` header, so the new order's id is unrecoverable.
    #[error("response missing Location header")]
    MissingLocationHeader,
    /// The `Location` header was present but did not parse into the
    /// expected `.../orders/{orderId}` shape.
    #[error("invalid Location header: {0}")]
    InvalidLocationHeader(String),
}

impl Error {
    /// Schwab-specific retry classification. Returns `true` for transient
    /// failures (network, 5xx, 429) where the same request can be safely
    /// retried by the caller. Returns `false` for terminal failures
    /// (4xx other than 429, decode errors, build errors).
    ///
    /// `schwab-rs` does not implement retry itself; this method exists so
    /// downstream consumers can utilize in their own retry logic.
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
            | Error::Encode { .. }
            | Error::Decode { .. }
            | Error::Build(_)
            | Error::MissingPreference(_)
            | Error::InvalidUri(_)
            | Error::MissingLocationHeader
            | Error::InvalidLocationHeader(_) => false,
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

/// The error body Schwab returns on 4xx/5xx responses.
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

/// Consume a non-2xx `reqwest::Response` and map it to the most specific
/// `Error` variant we can. Decodes the response body as a [`ServiceError`];
/// if the body isn't well-formed, falls back to [`Error::Decode`].
pub(crate) async fn map_response_to_error(response: reqwest::Response) -> Error {
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
