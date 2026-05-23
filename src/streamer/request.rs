use crate::CustomerId;
use crate::streamer::protocol::{Service, StreamerCommand};

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct RequestPayload {
    #[serde(rename = "requestid")]
    pub request_id: u64,
    #[serde(rename = "service")]
    pub service: Service,
    #[serde(rename = "command")]
    pub command: StreamerCommand,
    #[serde(rename = "parameters")]
    pub parameters: serde_json::Value,
    #[serde(rename = "SchwabClientCustomerId")]
    pub schwab_client_customer_id: CustomerId,
    #[serde(rename = "SchwabClientCorrelId")]
    pub schwab_client_correlation_id: String,
}

/// Crate-internal IR for a single outbound streamer command. Constructed
/// from typed builders (admin's `Login`/`Logout`, the per-service
/// `Subscription<F>`) via `From` impls in their respective modules, and
/// consumed only by [`crate::streamer::WriteHalf::send`].
pub(crate) struct StreamerRequest {
    pub(super) service: Service,
    pub(super) command: StreamerCommand,
    pub(super) parameters: serde_json::Value,
}
