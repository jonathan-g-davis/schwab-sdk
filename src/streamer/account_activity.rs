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
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::AccountActivity;
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
    use crate::streamer::StreamerRequest;
    use crate::streamer::StreamerResponse;
    use crate::streamer::response::{DataContent, parse};
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};

    #[test]
    fn parses_account_activity_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "ACCT_ACTIVITY",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "seq": 42,
                    "key": "my-correl-id",
                    "delayed": false,
                    "0": "my-correl-id",
                    "1": "12345678",
                    "2": "OrderEntryRequest",
                    "3": "{\"orderId\":\"ABC\",\"symbol\":\"AAPL\",\"quantity\":10}"
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::AccountActivity);
        let DataContent::AccountActivity(items) = &payload.content else {
            panic!("expected AccountActivity, got {:?}", payload.content);
        };
        let msg = &items[0];
        assert_eq!(msg.key, "my-correl-id");
        assert_eq!(msg.seq, Some(42));
        assert_eq!(msg.subscription_key.as_deref(), Some("my-correl-id"));
        assert_eq!(
            msg.account.as_ref().map(|a| a.expose_secret().to_string()),
            Some("12345678".to_string())
        );
        assert_eq!(msg.message_type.as_deref(), Some("OrderEntryRequest"));
        assert!(
            msg.message_data
                .as_deref()
                .map(|s| s.contains("AAPL"))
                .unwrap_or(false),
            "message_data should preserve raw payload"
        );
    }

    #[test]
    fn account_in_account_activity_redacts_on_debug() {
        // Compile-time check that Account is the redacted newtype.
        let frame = r#"{
            "data": [{
                "service": "ACCT_ACTIVITY",
                "timestamp": 1,
                "command": "SUBS",
                "content": [{
                    "seq": 1, "key": "k", "delayed": false,
                    "1": "12345678"
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let DataContent::AccountActivity(items) = &data[0].content else {
            panic!("expected AccountActivity");
        };
        let debug = format!("{:?}", items[0]);
        assert!(
            !debug.contains("12345678"),
            "account number leaked through Debug: {debug}"
        );
    }

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
