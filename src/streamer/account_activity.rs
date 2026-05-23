//! `ACCT_ACTIVITY` streamer service.
//!
//! Account-activity messages (order entries, fills, cancels, error messages,
//! etc.) for the account associated with the streamer session. Delivery type
//! "All Sequence": each message carries a `seq` field; reconnect-resend may
//! repeat a `seq` value, in which case the consumer may de-duplicate.
//!
//! `message_data` is intentionally exposed as an opaque `String`. Schwab
//! documents it as "either JSON-formatted data describing the update, NULL
//! in some cases, or plain text in case of ERROR" without enumerating the
//! variants per message type. Consumers route on `message_type` and parse
//! `message_data` themselves.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::{Error, Result};
use crate::secrets::AccountNumber;
use crate::streamer::{
    Service, StreamerRequest,
    subscription::{Subscription, subscribe_parameters},
};

impl From<Subscription<Field>> for StreamerRequest {
    fn from(subscription: Subscription<Field>) -> Self {
        StreamerRequest {
            service: Service::AccountActivity,
            command: subscription.command.into(),
            parameters: subscribe_parameters(subscription.keys, subscription.fields),
        }
    }
}

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
    SubscriptionKey,
    Account,
    MessageType,
    MessageData,
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

/// One ACCT_ACTIVITY message.
///
/// `account` is wrapped in `AccountNumber` (redacted `Debug`) because Schwab
/// account numbers are PII-equivalent. `message_data` is left as a `String`
/// so consumers can parse it according to `message_type`; do not log it
/// indiscriminately, as it may contain order details.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    pub key: String,
    pub delayed: bool,
    /// Sequence number. Reconnect-resend may repeat a value; consumers may
    /// de-duplicate.
    pub seq: Option<i64>,
    /// Field 0. Schwab echoes the subscription key passed in `SUBS`.
    pub subscription_key: Option<String>,
    /// Field 1. Schwab account number the activity occurred on.
    pub account: Option<AccountNumber>,
    /// Field 2. Identifier for the shape of `message_data`. Schwab leaves
    /// the set of values underspecified; route on this string rather than
    /// matching an exhaustive enum.
    pub message_type: Option<String>,
    /// Field 3. JSON-formatted payload, `null`, or plain text on error.
    pub message_data: Option<String>,
}

impl Content {
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "ACCT_ACTIVITY content".to_string(),
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["my-correl-id".to_string()],
            vec![
                Field::SubscriptionKey,
                Field::Account,
                Field::MessageType,
                Field::MessageData,
            ],
        );
        assert_eq!(value["keys"], "my-correl-id");
        assert_eq!(value["fields"], "0,1,2,3");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["my-correl-id".to_string()],
            fields: vec![Field::MessageType, Field::MessageData],
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
        assert_eq!(Field::SubscriptionKey.to_string(), "subscription_key");
        assert_eq!(Field::MessageType.to_string(), "message_type");
        assert_eq!(Field::MessageData.to_string(), "message_data");
    }
}
