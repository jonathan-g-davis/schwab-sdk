//! Crate-wide error type.
//!
//! Every fallible operation in `schwab-sdk` returns [`Result<T>`] aliasing
//! `std::result::Result<T, Error>`. Variants are kept structured wherever
//! Schwab gives us enough information; `Codec` carries a `context` string
//! describing the operation that failed.
//!
//! Non-2xx HTTP responses decode into an [`ErrorBody`]. Schwab's two API
//! families return different error envelopes - the Trader API a flat
//! [`ServiceError`], the Market Data API a structured [`ErrorResponse`] -
//! and [`ErrorBody`] preserves whichever arrived. A body matching neither
//! schema is kept verbatim so the HTTP status still maps to a typed
//! variant.
//!
//! [`Error::is_retryable`] and [`Error::retry_after`] are the only retry
//! seams the crate provides. Application code can use these to wire in
//! `backon` or another policy on top.
//!
//! # Examples
//!
//! Branch on whether a failure is worth retrying:
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, Error, SchwabClient};
//!
//! # async fn run() {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! match client.market_data().quotes().list(["AAPL"]).send().await {
//!     Ok(quotes) => println!("{} entries", quotes.len()),
//!     Err(err) if err.is_retryable() => println!("transient, safe to retry: {err}"),
//!     Err(Error::Unauthorized(_)) => println!("token rejected; refresh and retry"),
//!     Err(err) => println!("terminal: {err}"),
//! }
//! # }
//! ```

use std::time::Duration;

use http::StatusCode;
use serde_with::{DisplayFromStr, PickFirst, serde_as};

use crate::streamer;

/// Crate result alias: `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Error returned by every fallible operation in this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP 401. Distinct from [`Error::Http`] so a future token-refresh
    /// seam in `SchwabClient` has a single arm to hook.
    #[error("unauthorized: {0}")]
    Unauthorized(ErrorBody),
    /// HTTP 404. Distinct from [`Error::Http`] because callers idiomatically
    /// map "broker says no such resource" to `Ok(None)`.
    #[error("not found: {0}")]
    NotFound(ErrorBody),
    /// 429 with optional Retry-After.
    #[error("rate limited: {body}")]
    RateLimited {
        /// Parsed `Retry-After` header value, if Schwab sent one.
        retry_after: Option<Duration>,
        /// Decoded response body.
        body: ErrorBody,
    },
    /// Any other non-2xx response. The status is authoritative; the body
    /// is supplementary.
    #[error("http {status}: {body}")]
    Http {
        /// HTTP status from the response.
        status: StatusCode,
        /// Decoded response body.
        body: ErrorBody,
    },
    /// `reqwest` transport failure (DNS, connect, TLS, body read).
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    /// Streamer websocket: connect, handshake, or runtime frame error.
    #[error("websocket: {0}")]
    WebSocket(#[from] streamer::WebSocketError),
    /// JSON serde failure on a wire body or streamer frame. `context`
    /// names the operation (e.g. `"decode CHART_EQUITY frame"`,
    /// `"encode subscribe request"`).
    #[error("codec {context}: {reason}")]
    Codec {
        /// Names the operation that failed (e.g. `"decode response body"`).
        context: String,
        /// Underlying `serde` error message.
        reason: String,
    },
    /// `/userPreference` response missing a required field or carrying
    /// an unparseable value.
    #[error("invalid preference {field}: {reason}")]
    InvalidPreference {
        /// Name of the missing or unparseable field.
        field: &'static str,
        /// Why the field was rejected (e.g. `"missing"`, parse error text).
        reason: String,
    },
    /// Schwab acked a place / replace order but the `Location` header
    /// was absent or malformed, so the new order's id is unrecoverable.
    #[error("order id unrecoverable: {0}")]
    OrderIdUnrecoverable(String),
    /// A [`crate::TokenProvider`] failed to produce a bearer token, so no
    /// HTTP request could be issued. The wrapped source is the
    /// provider's own error type, type-erased; the SDK has no opinion on
    /// whether it is transient.
    #[error("token provider: {source}")]
    TokenProvider {
        /// Underlying provider error, type-erased.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// A base URL passed to [`crate::SchwabClient::with_trader_base_url`]
    /// or [`crate::SchwabClient::with_market_data_base_url`] used a
    /// scheme that is not permitted for the current build. Release
    /// builds require `https://`; debug builds additionally permit
    /// `http://` so local fixture servers (wiremock and similar) can be
    /// wired up in tests.
    #[error("insecure base url {url}: {reason}")]
    InsecureBaseUrl {
        /// The rejected URL string.
        url: String,
        /// Why the URL was rejected.
        reason: String,
    },
}

impl Error {
    /// Build the [`Error`] for a non-2xx HTTP status, given the decoded
    /// body and any `Retry-After` duration. The HTTP status is
    /// authoritative for the variant; the body is supplementary, so an
    /// unrecognized body still produces the correct status-based variant.
    pub(crate) fn from_status(
        status: StatusCode,
        retry_after: Option<Duration>,
        body: ErrorBody,
    ) -> Error {
        match status {
            StatusCode::UNAUTHORIZED => Error::Unauthorized(body),
            StatusCode::NOT_FOUND => Error::NotFound(body),
            StatusCode::TOO_MANY_REQUESTS => Error::RateLimited { retry_after, body },
            _ => Error::Http { status, body },
        }
    }

    /// Schwab-specific retry classification. Returns `true` for transient
    /// failures (network, 5xx, 429) where the same request can be safely
    /// retried by the caller. Returns `false` for terminal failures
    /// (4xx other than 429, codec errors, preference / location errors).
    ///
    /// `schwab-sdk` does not implement retry itself; this method exists
    /// so downstream consumers can utilize it in their own retry logic.
    ///
    /// # Examples
    ///
    /// A minimal backoff loop honoring [`Self::retry_after`] when present.
    /// In real code a crate such as `backon` is preferable; this shows the
    /// seam.
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use schwab_sdk::Result;
    ///
    /// async fn with_retry<F, Fut, T>(mut op: F) -> Result<T>
    /// where
    ///     F: FnMut() -> Fut,
    ///     Fut: std::future::Future<Output = Result<T>>,
    /// {
    ///     let mut attempts = 0;
    ///     loop {
    ///         match op().await {
    ///             Ok(value) => return Ok(value),
    ///             Err(err) if err.is_retryable() && attempts < 3 => {
    ///                 attempts += 1;
    ///                 let delay = err.retry_after().unwrap_or(Duration::from_millis(500));
    ///                 tokio::time::sleep(delay).await;
    ///             }
    ///             Err(err) => return Err(err),
    ///         }
    ///     }
    /// }
    ///
    /// # async fn caller(client: schwab_sdk::SchwabClient) -> Result<()> {
    /// let quotes = with_retry(|| client.market_data().quotes().list(["AAPL"]).send()).await?;
    /// # let _ = quotes;
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::RateLimited { .. } => true,
            Error::Http { status, .. } => status.is_server_error(),
            Error::Transport(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            Error::WebSocket(_) => true,
            Error::Unauthorized(_)
            | Error::NotFound(_)
            | Error::Codec { .. }
            | Error::InvalidPreference { .. }
            | Error::OrderIdUnrecoverable(_)
            | Error::TokenProvider { .. }
            | Error::InsecureBaseUrl { .. } => false,
        }
    }

    /// `Retry-After` duration parsed from a 429 response, when present.
    /// Always `None` for non-rate-limited errors.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Error::RateLimited { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

/// A decoded non-2xx response body.
///
/// Schwab's Trader and Market Data APIs return structurally different
/// error envelopes; this preserves whichever shape arrived.
///
/// # Examples
///
/// ```no_run
/// use schwab_sdk::{Error, ErrorBody};
///
/// # fn report(err: Error) {
/// match err {
///     Error::Http { status, body } => match body {
///         ErrorBody::Trader(svc) => eprintln!("{status}: {}", svc.message),
///         ErrorBody::MarketData(resp) => eprintln!("{status}: {resp}"),
///         ErrorBody::Unrecognized(raw) => eprintln!("{status}: {raw}"),
///     },
///     other => eprintln!("{other}"),
/// }
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorBody {
    /// Trader API shape: a top-level message plus error strings.
    Trader(ServiceError),
    /// Market Data API shape: a list of structured errors.
    MarketData(ErrorResponse),
    /// The body matched neither family's schema; the raw text is kept for
    /// diagnostics.
    Unrecognized(String),
}

impl ErrorBody {
    /// Decode a non-2xx response body.
    ///
    /// The two schemas are structurally disjoint: the Trader body
    /// requires a top-level `message` string with a `Vec<String>`
    /// `errors`, while the Market Data body has no `message` and an
    /// `errors` array of objects. A successful decode is therefore
    /// unambiguous. A body matching neither is returned as
    /// [`ErrorBody::Unrecognized`].
    pub(crate) fn parse(raw: &str) -> Self {
        if let Ok(trader) = serde_json::from_str::<ServiceError>(raw) {
            ErrorBody::Trader(trader)
        } else if let Ok(market_data) = serde_json::from_str::<ErrorResponse>(raw) {
            ErrorBody::MarketData(market_data)
        } else {
            ErrorBody::Unrecognized(raw.to_string())
        }
    }
}

impl std::fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorBody::Trader(e) => write!(f, "{e}"),
            ErrorBody::MarketData(e) => write!(f, "{e}"),
            ErrorBody::Unrecognized(raw) => write!(f, "{raw}"),
        }
    }
}

/// The error body Schwab's Trader API returns on 4xx/5xx responses.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ServiceError {
    /// Human-readable summary of the failure.
    pub message: String,
    /// Per-field or per-rule error messages; may be empty.
    #[serde(default)]
    pub errors: Vec<String>,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// The error body Schwab's Market Data API returns on 4xx/5xx responses:
/// a list of structured per-error entries.
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ErrorResponse {
    /// One entry per problem Schwab detected; empty if Schwab returned
    /// no structured detail.
    #[serde(default)]
    pub errors: Vec<ApiError>,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.errors.is_empty() {
            return write!(f, "no error detail");
        }
        for (i, error) in self.errors.iter().enumerate() {
            if i > 0 {
                write!(f, "; ")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

/// One structured error within an [`ErrorResponse`].
#[serde_as]
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ApiError {
    /// Unique error id Schwab assigns; useful when contacting support.
    #[serde(default)]
    pub id: Option<String>,
    /// HTTP status as Schwab echoes it in the body. Schwab is
    /// inconsistent about sending this as a JSON string or a JSON number;
    /// both decode here.
    #[serde(default)]
    #[serde_as(as = "Option<PickFirst<(_, DisplayFromStr)>>")]
    pub status: Option<u16>,
    /// Short error description.
    #[serde(default)]
    pub title: Option<String>,
    /// Detailed error description.
    #[serde(default)]
    pub detail: Option<String>,
    /// What in the request triggered the error.
    #[serde(default)]
    pub source: Option<ErrorSource>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.title, &self.detail) {
            (Some(title), Some(detail)) => write!(f, "{title}: {detail}"),
            (Some(title), None) => write!(f, "{title}"),
            (None, Some(detail)) => write!(f, "{detail}"),
            (None, None) => match &self.id {
                Some(id) => write!(f, "error {id}"),
                None => write!(f, "unspecified error"),
            },
        }
    }
}

/// Locates the request element that triggered an [`ApiError`].
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ErrorSource {
    /// JSON pointer(s) into the request body.
    #[serde(default)]
    pub pointer: Vec<String>,
    /// Query parameter name.
    #[serde(default)]
    pub parameter: Option<String>,
    /// Header name.
    #[serde(default)]
    pub header: Option<String>,
}

/// Consume a non-2xx `reqwest::Response` and map it to the most specific
/// [`Error`] variant. The body is decoded into an [`ErrorBody`]; a body
/// that decodes as neither family's schema is preserved verbatim rather
/// than discarded, so the status still drives the variant.
pub(crate) async fn map_response_to_error(response: reqwest::Response) -> Error {
    let status = response.status();
    let retry_after = parse_retry_after(response.headers());
    let raw = response
        .text()
        .await
        .unwrap_or_else(|e| format!("<error body unavailable: {e}>"));
    Error::from_status(status, retry_after, ErrorBody::parse(&raw))
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    value.parse::<u64>().ok().map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trader_error_body_parses() {
        let raw = r#"{
            "message": "Order validation failed",
            "errors": ["quantity must be positive", "symbol is required"]
        }"#;
        let ErrorBody::Trader(body) = ErrorBody::parse(raw) else {
            panic!("expected Trader body");
        };
        assert_eq!(body.message, "Order validation failed");
        assert_eq!(body.errors.len(), 2);
        assert_eq!(body.to_string(), "Order validation failed");
    }

    #[test]
    fn trader_error_body_without_errors_array_parses() {
        // The Trader schema marks `errors` optional; a body with only
        // `message` must still decode rather than degrading to Codec.
        let ErrorBody::Trader(body) = ErrorBody::parse(r#"{"message": "Forbidden"}"#) else {
            panic!("expected Trader body");
        };
        assert_eq!(body.message, "Forbidden");
        assert!(body.errors.is_empty());
    }

    #[test]
    fn market_data_error_body_parses() {
        // Modeled on Schwab's documented 400 response: three errors, each
        // with a different `source` locator and a string-valued `status`.
        let raw = r#"{
            "errors": [
                {
                    "id": "6808262e-52bb-4421-9d31-6c0e762e7dd5",
                    "status": "400",
                    "title": "Bad Request",
                    "detail": "Missing header",
                    "source": { "header": "Authorization" }
                },
                {
                    "id": "0be22ae7-efdf-44d9-99f4-f138049d76ca",
                    "status": "400",
                    "title": "Bad Request",
                    "detail": "Search combination should have min of 1.",
                    "source": { "pointer": ["/data/attributes/symbols", "/data/attributes/cusips"] }
                },
                {
                    "id": "28485414-290f-42e2-992b-58ea3e3203b1",
                    "status": "400",
                    "title": "Bad Request",
                    "detail": "valid fields should be any of all,fundamental,reference",
                    "source": { "parameter": "fields" }
                }
            ]
        }"#;
        let ErrorBody::MarketData(body) = ErrorBody::parse(raw) else {
            panic!("expected MarketData body");
        };
        assert_eq!(body.errors.len(), 3);

        let first = &body.errors[0];
        assert_eq!(first.status, Some(400));
        assert_eq!(first.title.as_deref(), Some("Bad Request"));
        assert_eq!(first.detail.as_deref(), Some("Missing header"));
        assert_eq!(
            first.source.as_ref().unwrap().header.as_deref(),
            Some("Authorization")
        );
        assert_eq!(first.to_string(), "Bad Request: Missing header");

        assert_eq!(body.errors[1].source.as_ref().unwrap().pointer.len(), 2);
        assert_eq!(
            body.errors[2].source.as_ref().unwrap().parameter.as_deref(),
            Some("fields")
        );
    }

    #[test]
    fn market_data_numeric_status_parses() {
        // Schwab's 401/404/500 examples send `status` as a bare number
        // rather than a string; it must still decode into `u16`.
        let raw = r#"{
            "errors": [
                { "id": "0be22ae7-efdf-44d9-99f4-f138049d76ca", "status": 401, "title": "Unauthorized" }
            ]
        }"#;
        let ErrorBody::MarketData(body) = ErrorBody::parse(raw) else {
            panic!("expected MarketData body");
        };
        assert_eq!(body.errors[0].status, Some(401));
        assert_eq!(body.errors[0].title.as_deref(), Some("Unauthorized"));
    }

    #[test]
    fn unrecognized_body_is_preserved() {
        // A plain-text upstream error (e.g. a gateway timeout page) must
        // not be discarded - the raw text is kept for diagnostics.
        let ErrorBody::Unrecognized(raw) = ErrorBody::parse("upstream request timeout") else {
            panic!("expected Unrecognized body");
        };
        assert_eq!(raw, "upstream request timeout");
    }

    #[test]
    fn trader_and_market_data_schemas_are_disjoint() {
        // The parse order relies on the two schemas not overlapping: a
        // Trader body must not decode as `ErrorResponse`, and vice versa.
        let trader = r#"{"message": "x", "errors": ["a"]}"#;
        let market_data = r#"{"errors": [{"status": 400, "title": "Bad Request"}]}"#;
        assert!(serde_json::from_str::<ErrorResponse>(trader).is_err());
        assert!(serde_json::from_str::<ServiceError>(market_data).is_err());
    }

    #[test]
    fn from_status_maps_each_documented_status() {
        let body = || ErrorBody::Unrecognized(String::new());
        assert!(matches!(
            Error::from_status(StatusCode::UNAUTHORIZED, None, body()),
            Error::Unauthorized(_)
        ));
        assert!(matches!(
            Error::from_status(StatusCode::NOT_FOUND, None, body()),
            Error::NotFound(_)
        ));
        assert!(matches!(
            Error::from_status(StatusCode::TOO_MANY_REQUESTS, None, body()),
            Error::RateLimited { .. }
        ));
        assert!(matches!(
            Error::from_status(StatusCode::BAD_REQUEST, None, body()),
            Error::Http { status, .. } if status == StatusCode::BAD_REQUEST
        ));
        assert!(matches!(
            Error::from_status(StatusCode::FORBIDDEN, None, body()),
            Error::Http { status, .. } if status == StatusCode::FORBIDDEN
        ));
        assert!(matches!(
            Error::from_status(StatusCode::SERVICE_UNAVAILABLE, None, body()),
            Error::Http { status, .. } if status == StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(matches!(
            Error::from_status(StatusCode::INTERNAL_SERVER_ERROR, None, body()),
            Error::Http { status, .. } if status == StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(matches!(
            Error::from_status(StatusCode::BAD_GATEWAY, None, body()),
            Error::Http { status, .. } if status == StatusCode::BAD_GATEWAY
        ));
    }

    #[test]
    fn rate_limited_carries_retry_after_and_is_retryable() {
        let error = Error::from_status(
            StatusCode::TOO_MANY_REQUESTS,
            Some(Duration::from_secs(30)),
            ErrorBody::Unrecognized(String::new()),
        );
        assert_eq!(error.retry_after(), Some(Duration::from_secs(30)));
        assert!(error.is_retryable());
    }

    #[test]
    fn client_errors_are_not_retryable() {
        let body = || ErrorBody::Unrecognized(String::new());
        assert!(!Error::from_status(StatusCode::BAD_REQUEST, None, body()).is_retryable());
        assert!(!Error::from_status(StatusCode::NOT_FOUND, None, body()).is_retryable());
        assert!(!Error::from_status(StatusCode::UNAUTHORIZED, None, body()).is_retryable());
        assert!(Error::from_status(StatusCode::INTERNAL_SERVER_ERROR, None, body()).is_retryable());
        assert!(Error::from_status(StatusCode::BAD_GATEWAY, None, body()).is_retryable());
    }
}
