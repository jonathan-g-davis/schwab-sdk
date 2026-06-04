//! `CHART_EQUITY` streamer service.
//!
//! Minute OHLCV candles for equities. Delivery type "All Sequence": every
//! tick from the source is forwarded with a sequence number; the streamer
//! does not conflate.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::{Error, Result};
use crate::serde_time::{epoch_days_opt, millis_opt};
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::ChartEquity;
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
/// Numbered subscription field for CHART_EQUITY.
///
/// The field numbers match the live wire format, which differs from
/// Schwab's published streaming documentation. Field 1 is the sequence number
/// and Field 6 is the volume.
#[non_exhaustive]
pub enum Field {
    /// Field 0. Schwab labels this `"key"` in their docs; we expose it as
    /// `Symbol` so the snake_case key (`symbol`) does not collide with the
    /// top-level `"key"` field that always carries the ticker.
    Symbol,
    /// Schwab-assigned sequence number; identifies the candle minute (field 1).
    Sequence,
    /// Candle open, USD (field 2).
    OpenPrice,
    /// Candle high, USD (field 3).
    HighPrice,
    /// Candle low, USD (field 4).
    LowPrice,
    /// Candle close, USD (field 5).
    ClosePrice,
    /// Candle volume; may be fractional (field 6).
    Volume,
    /// Candle-open timestamp, epoch milliseconds (field 7).
    ChartTime,
    /// Days since epoch (field 8).
    ChartDay,
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

/// One minute OHLCV candle.
///
/// `volume` is `Decimal` because Schwab types it as a `double` on the wire
/// (fractional share volume on some venues). Timestamps are milliseconds
/// since the Unix epoch.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the symbol).
    pub key: String,
    /// `true` if the candle is delayed.
    pub delayed: bool,
    /// Asset class string (`"EQUITY"`).
    #[serde(rename = "assetMainType")]
    pub asset_main_type: Option<String>,
    /// Asset sub-type string.
    #[serde(rename = "assetSubType")]
    pub asset_sub_type: Option<String>,
    /// CUSIP, when Schwab supplies one.
    pub cusip: Option<String>,

    /// Field 0: wire symbol.
    pub symbol: Option<String>,
    /// Field 1: Schwab-assigned sequence number.
    pub sequence: Option<i64>,
    /// Field 2: candle open, USD.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Field 3: candle high, USD.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Field 4: candle low, USD.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Field 5: candle close, USD.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Field 6: candle volume; may be fractional on some venues.
    #[serde(with = "decimal_opt")]
    pub volume: Option<Decimal>,
    /// Field 7: candle-open timestamp.
    #[serde(with = "millis_opt")]
    pub chart_time: Option<DateTime<Utc>>,
    /// Field 8: candle-open date (decoded from days since epoch).
    #[serde(with = "epoch_days_opt")]
    pub chart_day: Option<NaiveDate>,
}

impl Content {
    /// Decode a remapped JSON object (numeric keys already resolved to
    /// snake_case names by the streamer frame parser) into a typed batch.
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "CHART_EQUITY content".to_string(),
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
    fn parses_chart_equity_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "CHART_EQUITY",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL",
                    "delayed": false,
                    "1": 1234,
                    "2": 183.50, "3": 183.80, "4": 183.45, "5": 183.75,
                    "6": 125000,
                    "7": 1714949580000,
                    "8": 19850
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::ChartEquity);
        let DataContent::ChartEquity(items) = &payload.content else {
            panic!("expected ChartEquity, got {:?}", payload.content);
        };
        let candle = &items[0];
        assert_eq!(candle.key, "AAPL");
        assert_eq!(candle.open_price, Some(dec!(183.50)));
        assert_eq!(candle.high_price, Some(dec!(183.80)));
        assert_eq!(candle.low_price, Some(dec!(183.45)));
        assert_eq!(candle.close_price, Some(dec!(183.75)));
        assert_eq!(candle.volume, Some(dec!(125000)));
        assert_eq!(candle.sequence, Some(1234));
        assert_eq!(candle.chart_time.unwrap().timestamp_millis(), 1714949580000);
        // 19850 days since the Unix epoch is 2024-05-07.
        assert_eq!(candle.chart_day, NaiveDate::from_ymd_opt(2024, 5, 7));
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["AAPL".to_string()],
            vec![
                Field::OpenPrice,
                Field::HighPrice,
                Field::LowPrice,
                Field::ClosePrice,
                Field::Volume,
                Field::ChartTime,
            ],
        );
        assert_eq!(value["keys"], "AAPL");
        assert_eq!(value["fields"], "2,3,4,5,6,7");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["AAPL".to_string(), "MSFT".to_string()],
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
        assert_eq!(Field::OpenPrice.to_string(), "open_price");
        assert_eq!(Field::ChartTime.to_string(), "chart_time");
        assert_eq!(Field::ChartDay.to_string(), "chart_day");
    }
}
