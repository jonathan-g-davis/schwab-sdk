//! `OPTIONS_BOOK` streamer service.
//!
//! Level-2 order book for listed options. See `book::Content`,
//! `book::PriceLevel`, and `book::MarketMaker` for the shared payload shape.
//!
//! Note: distinct from `crate::streamer::level_one::options`, which is the
//! Level-1 quote stream for the same instruments.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::book;
use crate::streamer::{
    Service, StreamerRequest,
    subscription::{Subscription, subscribe_parameters},
};

impl From<Subscription<Field>> for StreamerRequest {
    fn from(subscription: Subscription<Field>) -> Self {
        StreamerRequest {
            service: Service::OptionsBook,
            command: subscription.command.into(),
            parameters: subscribe_parameters(subscription.keys, subscription.fields),
        }
    }
}

/// Field enum for the OPTIONS_BOOK service. Identical layout to NYSE_BOOK and
/// NASDAQ_BOOK but a distinct type so the `From<Subscription<Field>>` impl
/// can pick the right `Service` variant.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    serde_repr::Serialize_repr,
    Display,
    EnumString,
    FromRepr,
)]
#[repr(u8)]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum Field {
    Symbol,
    MarketSnapshotTime,
    BidSideLevels,
    AskSideLevels,
}

impl From<Field> for u8 {
    fn from(field: Field) -> Self {
        field as u8
    }
}

impl TryFrom<u8> for Field {
    type Error = String;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        Field::from_repr(value).ok_or_else(|| format!("Invalid field: {}", value))
    }
}

pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<book::Content>> {
    book::decode_batch(remapped, "OPTIONS_BOOK")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["AAPL  240315C00200000".to_string()],
            vec![
                Field::Symbol,
                Field::MarketSnapshotTime,
                Field::BidSideLevels,
                Field::AskSideLevels,
            ],
        );
        assert_eq!(value["keys"], "AAPL  240315C00200000");
        assert_eq!(value["fields"], "0,1,2,3");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["AAPL  240315C00200000".to_string()],
            fields: vec![Field::BidSideLevels, Field::AskSideLevels],
        };
        let _request: StreamerRequest = sub.into();

        let sub = Subscription::<Field> {
            command: Command::Unsubscribe,
            keys: vec![],
            fields: vec![],
        };
        let _request: StreamerRequest = sub.into();
    }

    #[test]
    fn snake_case_field_names_round_trip() {
        assert_eq!(
            Field::MarketSnapshotTime.to_string(),
            "market_snapshot_time"
        );
        assert_eq!(Field::BidSideLevels.to_string(), "bid_side_levels");
        assert_eq!(Field::AskSideLevels.to_string(), "ask_side_levels");
    }
}
