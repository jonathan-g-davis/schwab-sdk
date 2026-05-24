//! `SCREENER_EQUITY` streamer service.
//!
//! See `screener::Content`, `screener::Item` for the shared payload shape.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::screener;
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::ScreenerEquity;
}

/// Field enum for the SCREENER_EQUITY service. Identical layout to
/// SCREENER_OPTION but distinct so the `SubscriptionField` impl can bind the
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
    use crate::streamer::StreamerRequest;
    use crate::streamer::StreamerResponse;
    use crate::streamer::response::{DataContent, parse};
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};
    use rust_decimal_macros::dec;

    #[test]
    fn parses_screener_equity_data_into_typed_content() {
        // Two-item ranking on NYSE volume, 5-minute window. Items carry
        // camelCase named fields per Schwab's spec.
        let frame = r#"{
            "data": [{
                "service": "SCREENER_EQUITY",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "NYSE_VOLUME_5",
                    "delayed": false,
                    "1": 1714949590000,
                    "2": "VOLUME",
                    "3": 5,
                    "4": [
                        {
                            "description": "Apple Inc.",
                            "lastPrice": 183.50,
                            "marketShare": 1.25,
                            "netChange": 0.75,
                            "netPercentChange": 0.4106,
                            "symbol": "AAPL",
                            "totalVolume": 163224109,
                            "trades": 95012,
                            "volume": 12500000
                        },
                        {
                            "description": "Microsoft Corp.",
                            "lastPrice": 425.10,
                            "marketShare": 0.85,
                            "netChange": -1.20,
                            "netPercentChange": -0.2814,
                            "symbol": "MSFT",
                            "totalVolume": 22500000,
                            "trades": 41200,
                            "volume": 7250000
                        }
                    ]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ScreenerEquity);
        let DataContent::ScreenerEquity(rows) = &payload.content else {
            panic!("expected ScreenerEquity, got {:?}", payload.content);
        };
        let row = &rows[0];
        assert_eq!(row.key, "NYSE_VOLUME_5");
        assert_eq!(row.timestamp, Some(1714949590000));
        assert_eq!(row.sort_field.as_deref(), Some("VOLUME"));
        assert_eq!(row.frequency, Some(5));
        assert_eq!(row.items.len(), 2);

        let aapl = &row.items[0];
        assert_eq!(aapl.symbol.as_deref(), Some("AAPL"));
        assert_eq!(aapl.description.as_deref(), Some("Apple Inc."));
        assert_eq!(aapl.last_price, Some(dec!(183.50)));
        assert_eq!(aapl.market_share, Some(dec!(1.25)));
        assert_eq!(aapl.net_change, Some(dec!(0.75)));
        assert_eq!(aapl.net_percent_change, Some(dec!(0.4106)));
        assert_eq!(aapl.total_volume, Some(163224109));
        assert_eq!(aapl.trades, Some(95012));
        assert_eq!(aapl.volume, Some(12500000));

        let msft = &row.items[1];
        assert_eq!(msft.symbol.as_deref(), Some("MSFT"));
        assert_eq!(msft.net_change, Some(dec!(-1.20)));
    }

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
