//! `LEVELONE_FUTURES_OPTIONS` streamer service.
//!
//! Delivery type: Change. Fields not present on a tick stay `None`.
//!
//! Futures-options symbols are Schwab-standard: `./` + root + month + year +
//! `C`/`P` + strike (e.g. `./OZCZ23C565`).

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
            service: Service::LevelOneFuturesOptions,
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
    BidId,
    AskId,
    TotalVolume,
    LastSize,
    QuoteTime,
    TradeTime,
    HighPrice,
    LowPrice,
    ClosePrice,
    LastId,
    Description,
    OpenPrice,
    OpenInterest,
    Mark,
    Tick,
    TickAmount,
    FutureMultiplier,
    FutureSettlementPrice,
    UnderlyingSymbol,
    StrikePrice,
    FutureExpirationDate,
    ExpirationStyle,
    ContractType,
    SecurityStatus,
    Exchange,
    ExchangeName,
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

/// Typed payload for a single LEVELONE_FUTURES_OPTIONS update.
///
/// **Note**: per Schwab's docs, field 18 (`open_interest`) is `double` for
/// futures-options (unlike LEVELONE_OPTIONS where it's `int`). We honor the
/// spec and use `Option<Decimal>`.
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
    // Field 6 - "?" for unknown; all quotes currently CME.
    pub bid_id: Option<String>,
    // Field 7
    pub ask_id: Option<String>,
    // Field 8
    pub total_volume: Option<u64>,
    // Field 9
    pub last_size: Option<u64>,
    // Field 10
    pub quote_time: Option<u64>,
    // Field 11
    pub trade_time: Option<u64>,
    // Field 12
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    // Field 13
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    // Field 14
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    // Field 15
    pub last_id: Option<String>,
    // Field 16
    pub description: Option<String>,
    // Field 17
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    // Field 18 - Schwab spec types this as `double` for futures-options.
    #[serde(with = "decimal_opt")]
    pub open_interest: Option<Decimal>,
    // Field 19
    #[serde(with = "decimal_opt")]
    pub mark: Option<Decimal>,
    // Field 20
    #[serde(with = "decimal_opt")]
    pub tick: Option<Decimal>,
    // Field 21
    #[serde(with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    // Field 22
    #[serde(with = "decimal_opt")]
    pub future_multiplier: Option<Decimal>,
    // Field 23
    #[serde(with = "decimal_opt")]
    pub future_settlement_price: Option<Decimal>,
    // Field 24
    pub underlying_symbol: Option<String>,
    // Field 25
    #[serde(with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    // Field 26 - ms since Unix epoch.
    pub future_expiration_date: Option<i64>,
    // Field 27
    pub expiration_style: Option<String>,
    // Field 28 - C / P.
    pub contract_type: Option<String>,
    // Field 29
    pub security_status: Option<String>,
    // Field 30
    pub exchange: Option<String>,
    // Field 31
    pub exchange_name: Option<String>,
}

impl Content {
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "LEVELONE_FUTURES_OPTIONS content".to_string(),
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
            vec!["./OZCZ23C565".to_string()],
            vec![
                Field::Symbol,
                Field::BidPrice,
                Field::StrikePrice,
                Field::ContractType,
            ],
        );
        assert_eq!(value["keys"], "./OZCZ23C565");
        assert_eq!(value["fields"], "0,1,25,28");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["./OZCZ23C565".to_string()],
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
        assert_eq!(Field::UnderlyingSymbol.to_string(), "underlying_symbol");
        assert_eq!(
            Field::FutureExpirationDate.to_string(),
            "future_expiration_date"
        );
        assert_eq!(Field::ContractType.to_string(), "contract_type");
    }
}
