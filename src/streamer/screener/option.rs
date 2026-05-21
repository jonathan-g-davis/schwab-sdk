//! `SCREENER_OPTION` streamer service.
//!
//! See `screener::Content`, `screener::Item` for the shared payload shape.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::screener;
use crate::streamer::{
    Service, StreamerRequest,
    subscription::{Subscription, SubscriptionParameters},
};

impl From<Subscription<Field>> for StreamerRequest {
    fn from(subscription: Subscription<Field>) -> Self {
        let parameters = serde_json::to_value(SubscriptionParameters {
            keys: subscription.keys,
            fields: subscription.fields,
        })
        .expect("SubscriptionParameters serialization is infallible");
        StreamerRequest {
            service: Service::ScreenerOption,
            command: subscription.command.into(),
            parameters,
        }
    }
}

/// Field enum for the SCREENER_OPTION service. Identical layout to
/// SCREENER_EQUITY but distinct so the `From<Subscription<Field>>` impl picks
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
    screener::decode_batch(remapped, "SCREENER_OPTION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::subscription::{Command, Subscription};

    #[test]
    fn fields_serialize_as_numeric_index() {
        let params = SubscriptionParameters {
            keys: vec!["OPTION_CALL_VOLUME_5".to_string()],
            fields: vec![
                Field::Symbol,
                Field::Timestamp,
                Field::SortField,
                Field::Frequency,
                Field::Items,
            ],
        };
        let serialized = serde_json::to_string(&params).unwrap();
        assert_eq!(
            serialized,
            r#"{"keys":"OPTION_CALL_VOLUME_5","fields":"0,1,2,3,4"}"#
        );
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec![
                "OPTION_CALL_VOLUME_5".to_string(),
                "OPTION_ALL_PERCENT_CHANGE_UP_1".to_string(),
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
