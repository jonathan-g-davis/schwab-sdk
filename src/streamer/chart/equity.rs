//! `CHART_EQUITY` streamer service.
//!
//! Minute OHLCV candles for equities. Delivery type "All Sequence": every
//! tick from the source is forwarded with a sequence number; the streamer
//! does not conflate.

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::{Error, Result};
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
            service: Service::ChartEquity,
            command: subscription.command.into(),
            parameters,
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
    /// Field 0. Schwab labels this `"key"` in their docs; we expose it as
    /// `Symbol` so the snake_case key (`symbol`) does not collide with the
    /// top-level `"key"` field that always carries the ticker.
    Symbol,
    OpenPrice,
    HighPrice,
    LowPrice,
    ClosePrice,
    Volume,
    Sequence,
    ChartTime,
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
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    pub key: String,
    pub delayed: bool,
    #[serde(rename = "assetMainType")]
    pub asset_main_type: Option<String>,
    #[serde(rename = "assetSubType")]
    pub asset_sub_type: Option<String>,
    pub cusip: Option<String>,

    // Field 0.
    pub symbol: Option<String>,
    // Field 1.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    // Field 2.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    // Field 3.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    // Field 4.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    // Field 5.
    #[serde(with = "decimal_opt")]
    pub volume: Option<Decimal>,
    // Field 6.
    pub sequence: Option<i64>,
    // Field 7. Milliseconds since the Unix epoch.
    pub chart_time: Option<u64>,
    // Field 8.
    pub chart_day: Option<i32>,
}

impl Content {
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Decode {
            context: "CHART_EQUITY content".to_string(),
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::subscription::{Command, Subscription};

    #[test]
    fn fields_serialize_as_numeric_index() {
        let params = SubscriptionParameters {
            keys: vec!["AAPL".to_string()],
            fields: vec![
                Field::OpenPrice,
                Field::HighPrice,
                Field::LowPrice,
                Field::ClosePrice,
                Field::Volume,
                Field::ChartTime,
            ],
        };
        let serialized = serde_json::to_string(&params).unwrap();
        assert_eq!(serialized, r#"{"keys":"AAPL","fields":"1,2,3,4,5,7"}"#);
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
