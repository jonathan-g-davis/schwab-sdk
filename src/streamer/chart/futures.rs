//! `CHART_FUTURES` streamer service.
//!
//! Minute OHLCV candles for futures. Delivery type "All Sequence". Field
//! ordering differs from CHART_EQUITY: `chart_time` is field 1 here, not 7,
//! and there is no `sequence` or `chart_day` field.

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::{Error, Result};
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::ChartFutures;
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
/// Numbered subscription field for CHART_FUTURES.
#[non_exhaustive]
pub enum Field {
    /// Field 0. Renamed from Schwab's `"key"` label so the snake_case key
    /// (`symbol`) does not collide with the top-level `"key"` field.
    Symbol,
    /// Candle-open timestamp, epoch milliseconds (field 1).
    ChartTime,
    /// Candle open (field 2).
    OpenPrice,
    /// Candle high (field 3).
    HighPrice,
    /// Candle low (field 4).
    LowPrice,
    /// Candle close (field 5).
    ClosePrice,
    /// Candle volume, contracts (field 6).
    Volume,
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

/// One minute OHLCV candle for a futures contract.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the futures symbol).
    pub key: String,
    /// `true` if the candle is delayed.
    pub delayed: bool,
    /// Asset class string (`"FUTURE"`).
    #[serde(rename = "assetMainType")]
    pub asset_main_type: Option<String>,
    /// Asset sub-type string.
    #[serde(rename = "assetSubType")]
    pub asset_sub_type: Option<String>,
    /// CUSIP, when Schwab supplies one.
    pub cusip: Option<String>,

    /// Field 0: wire symbol.
    pub symbol: Option<String>,
    /// Field 1: candle-open timestamp, epoch milliseconds.
    pub chart_time: Option<u64>,
    /// Field 2: candle open.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Field 3: candle high.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Field 4: candle low.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Field 5: candle close.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Field 6: candle volume, contracts.
    #[serde(with = "decimal_opt")]
    pub volume: Option<Decimal>,
}

impl Content {
    /// Decode a remapped JSON object (numeric keys already resolved to
    /// snake_case names by the streamer frame parser) into a typed batch.
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "CHART_FUTURES content".to_string(),
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
    use rust_decimal_macros::dec;

    #[test]
    fn parses_chart_futures_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "CHART_FUTURES",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "/ESZ24",
                    "delayed": false,
                    "1": 1714949580000,
                    "2": 5020.00, "3": 5025.50, "4": 5018.25, "5": 5024.75,
                    "6": 8520
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ChartFutures);
        let DataContent::ChartFutures(items) = &payload.content else {
            panic!("expected ChartFutures, got {:?}", payload.content);
        };
        let candle = &items[0];
        assert_eq!(candle.key, "/ESZ24");
        assert_eq!(candle.chart_time, Some(1714949580000));
        assert_eq!(candle.open_price, Some(dec!(5020.00)));
        assert_eq!(candle.high_price, Some(dec!(5025.50)));
        assert_eq!(candle.low_price, Some(dec!(5018.25)));
        assert_eq!(candle.close_price, Some(dec!(5024.75)));
        assert_eq!(candle.volume, Some(dec!(8520)));
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["/ESZ24".to_string()],
            vec![
                Field::ChartTime,
                Field::OpenPrice,
                Field::HighPrice,
                Field::LowPrice,
                Field::ClosePrice,
                Field::Volume,
            ],
        );
        assert_eq!(value["keys"], "/ESZ24");
        assert_eq!(value["fields"], "1,2,3,4,5,6");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["/ESZ24".to_string(), "/NQZ24".to_string()],
            fields: vec![Field::OpenPrice, Field::ClosePrice],
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
        assert_eq!(Field::ChartTime.to_string(), "chart_time");
        assert_eq!(Field::OpenPrice.to_string(), "open_price");
        assert_eq!(Field::ClosePrice.to_string(), "close_price");
    }
}
