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
use crate::streamer::{
    Service, StreamerRequest,
    subscription::{Subscription, subscribe_parameters},
};

impl From<Subscription<Field>> for StreamerRequest {
    fn from(subscription: Subscription<Field>) -> Self {
        StreamerRequest {
            service: Service::ChartFutures,
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
    /// Field 0. Renamed from Schwab's `"key"` label so the snake_case key
    /// (`symbol`) does not collide with the top-level `"key"` field.
    Symbol,
    ChartTime,
    OpenPrice,
    HighPrice,
    LowPrice,
    ClosePrice,
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
    // Field 1. Milliseconds since the Unix epoch.
    pub chart_time: Option<u64>,
    // Field 2.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    // Field 3.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    // Field 4.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    // Field 5.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    // Field 6.
    #[serde(with = "decimal_opt")]
    pub volume: Option<Decimal>,
}

impl Content {
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
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};

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
