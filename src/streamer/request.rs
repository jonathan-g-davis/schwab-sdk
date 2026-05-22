use crate::streamer::{account_activity, admin, book, chart, level_one, screener, subscription};
use crate::{
    CustomerId,
    streamer::protocol::{Service, StreamerCommand},
};

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

pub struct StreamerRequest {
    pub(super) service: Service,
    pub(super) command: StreamerCommand,
    pub(super) parameters: serde_json::Value,
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

    pub fn futures_options() -> subscription::SubscriptionBuilder<level_one::futures_options::Field>
    {
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
