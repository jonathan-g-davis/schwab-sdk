//! `LEVELONE_FOREX` streamer service.
//!
//! Delivery type: Change. Fields not present on a tick stay `None`.
//!
//! Forex symbols are Schwab-standard pair notation: `EUR/USD`, `USD/JPY`, etc.

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
            service: Service::LevelOneForex,
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
    BidPrice,
    AskPrice,
    LastPrice,
    BidSize,
    AskSize,
    TotalVolume,
    LastSize,
    QuoteTime,
    TradeTime,
    HighPrice,
    LowPrice,
    ClosePrice,
    Exchange,
    Description,
    OpenPrice,
    NetChange,
    PercentChange,
    ExchangeName,
    Digits,
    SecurityStatus,
    Tick,
    TickAmount,
    Product,
    TradingHours,
    IsTradable,
    MarketMaker,
    High52Week,
    Low52Week,
    Mark,
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

/// Typed payload for a single LEVELONE_FOREX update.
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

    // Field 0
    pub symbol: Option<String>,
    // Field 1
    #[serde(with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    // Field 2
    #[serde(with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    // Field 3
    #[serde(with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    // Field 4
    pub bid_size: Option<u64>,
    // Field 5
    pub ask_size: Option<u64>,
    // Field 6
    pub total_volume: Option<u64>,
    // Field 7
    pub last_size: Option<u64>,
    // Field 8
    pub quote_time: Option<u64>,
    // Field 9
    pub trade_time: Option<u64>,
    // Field 10
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    // Field 11
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    // Field 12
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    // Field 13
    pub exchange: Option<String>,
    // Field 14
    pub description: Option<String>,
    // Field 15
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    // Field 16
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    // Field 17 - if(close > 0): (last - close) / close, else 0.
    #[serde(with = "decimal_opt")]
    pub percent_change: Option<Decimal>,
    // Field 18
    pub exchange_name: Option<String>,
    // Field 19
    pub digits: Option<i32>,
    // Field 20
    pub security_status: Option<String>,
    // Field 21
    #[serde(with = "decimal_opt")]
    pub tick: Option<Decimal>,
    // Field 22
    #[serde(with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    // Field 23
    pub product: Option<String>,
    // Field 24
    pub trading_hours: Option<String>,
    // Field 25
    pub is_tradable: Option<bool>,
    // Field 26
    pub market_maker: Option<String>,
    // Field 27
    #[serde(with = "decimal_opt")]
    pub high52_week: Option<Decimal>,
    // Field 28
    #[serde(with = "decimal_opt")]
    pub low52_week: Option<Decimal>,
    // Field 29 - mark-to-market value.
    #[serde(with = "decimal_opt")]
    pub mark: Option<Decimal>,
}

impl Content {
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "LEVELONE_FOREX content".to_string(),
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
            keys: vec!["EUR/USD".to_string()],
            fields: vec![Field::Symbol, Field::BidPrice, Field::AskPrice, Field::Mark],
        };
        let serialized = serde_json::to_string(&params).unwrap();
        assert_eq!(serialized, r#"{"keys":"EUR/USD","fields":"0,1,2,29"}"#);
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["EUR/USD".to_string(), "USD/JPY".to_string()],
            fields: vec![Field::Symbol, Field::BidPrice],
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
        assert_eq!(Field::High52Week.to_string(), "high52_week");
        assert_eq!(Field::Low52Week.to_string(), "low52_week");
        assert_eq!(Field::PercentChange.to_string(), "percent_change");
        assert_eq!(Field::IsTradable.to_string(), "is_tradable");
        assert_eq!(Field::MarketMaker.to_string(), "market_maker");
    }
}
