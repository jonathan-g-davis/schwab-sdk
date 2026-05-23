//! `SCREENER_EQUITY` streamer service.
//!
//! See `screener::Content`, `screener::Item` for the shared payload shape.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::screener;
use crate::streamer::{
    Service, StreamerRequest,
    subscription::{Subscription, subscribe_parameters},
};

impl From<Subscription<Field>> for StreamerRequest {
    fn from(subscription: Subscription<Field>) -> Self {
        StreamerRequest {
            service: Service::ScreenerEquity,
            command: subscription.command.into(),
            parameters: subscribe_parameters(subscription.keys, subscription.fields),
        }
    }
}

/// Field enum for the SCREENER_EQUITY service. Identical layout to
/// SCREENER_OPTION but distinct so the `From<Subscription<Field>>` impl picks
/// the correct `Service`.
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
    Timestamp,
    SortField,
    Frequency,
    Items,
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

pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<screener::Content>> {
    screener::decode_batch(remapped, "SCREENER_EQUITY")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["NYSE_VOLUME_5".to_string()],
            vec![
                Field::Symbol,
                Field::Timestamp,
                Field::SortField,
                Field::Frequency,
                Field::Items,
            ],
        );
        assert_eq!(value["keys"], "NYSE_VOLUME_5");
        assert_eq!(value["fields"], "0,1,2,3,4");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec![
                "NYSE_VOLUME_5".to_string(),
                "EQUITY_ALL_PERCENT_CHANGE_UP_1".to_string(),
            ],
            fields: vec![Field::Items],
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
        assert_eq!(Field::SortField.to_string(), "sort_field");
        assert_eq!(Field::Items.to_string(), "items");
        assert_eq!(Field::Frequency.to_string(), "frequency");
    }
}
