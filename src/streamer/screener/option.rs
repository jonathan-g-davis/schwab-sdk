//! `SCREENER_OPTION` streamer service.
//!
//! See `screener::Content`, `screener::Item` for the shared payload shape.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::screener;
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::ScreenerOption;
}

/// Field enum for the SCREENER_OPTION service. Identical layout to
/// SCREENER_EQUITY but distinct so the `SubscriptionField` impl can bind the
/// correct `Service` variant.
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
    /// Wire symbol / composite screener key (field 0).
    Symbol,
    /// Snapshot timestamp, epoch milliseconds (field 1).
    Timestamp,
    /// Field the rankings were sorted on (field 2).
    SortField,
    /// Aggregation window in minutes (field 3).
    Frequency,
    /// Ranked instruments (field 4).
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
    use crate::streamer::StreamerRequest;
    use crate::streamer::StreamerResponse;
    use crate::streamer::response::{DataContent, parse};
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};
    use rust_decimal_macros::dec;

    #[test]
    fn parses_screener_option_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "SCREENER_OPTION",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "OPTION_CALL_VOLUME_5",
                    "delayed": false,
                    "1": 1714949590000,
                    "2": "VOLUME",
                    "3": 5,
                    "4": [{
                        "description": "AAPL Mar 15 2024 200 Call",
                        "lastPrice": 5.15,
                        "marketShare": 0.40,
                        "netChange": 0.05,
                        "netPercentChange": 0.9804,
                        "symbol": "AAPL  240315C00200000",
                        "totalVolume": 12345,
                        "trades": 312,
                        "volume": 8400
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ScreenerOption);
        let DataContent::ScreenerOption(rows) = &payload.content else {
            panic!("expected ScreenerOption, got {:?}", payload.content);
        };
        let row = &rows[0];
        assert_eq!(row.key, "OPTION_CALL_VOLUME_5");
        assert_eq!(row.sort_field.as_deref(), Some("VOLUME"));
        assert_eq!(row.frequency, Some(5));
        assert_eq!(row.items.len(), 1);

        let item = &row.items[0];
        assert_eq!(item.symbol.as_deref(), Some("AAPL  240315C00200000"));
        assert_eq!(item.last_price, Some(dec!(5.15)));
        assert_eq!(item.net_change, Some(dec!(0.05)));
        assert_eq!(item.volume, Some(8400));
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["OPTION_CALL_VOLUME_5".to_string()],
            vec![
                Field::Symbol,
                Field::Timestamp,
                Field::SortField,
                Field::Frequency,
                Field::Items,
            ],
        );
        assert_eq!(value["keys"], "OPTION_CALL_VOLUME_5");
        assert_eq!(value["fields"], "0,1,2,3,4");
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
