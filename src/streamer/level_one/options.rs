//! `LEVELONE_OPTIONS` streamer service.
//!
//! Delivery type: Change. Fields not present on a tick stay `None`.

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
            service: Service::LevelOneOptions,
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
    Description,
    BidPrice,
    AskPrice,
    LastPrice,
    HighPrice,
    LowPrice,
    ClosePrice,
    TotalVolume,
    OpenInterest,
    Volatility,
    MoneyIntrinsicValue,
    ExpirationYear,
    Multiplier,
    Digits,
    OpenPrice,
    BidSize,
    AskSize,
    LastSize,
    NetChange,
    StrikePrice,
    ContractType,
    Underlying,
    ExpirationMonth,
    Deliverables,
    TimeValue,
    ExpirationDay,
    DaysToExpiration,
    Delta,
    Gamma,
    Theta,
    Vega,
    Rho,
    SecurityStatus,
    TheoreticalOptionValue,
    UnderlyingPrice,
    UvExpirationType,
    MarkPrice,
    QuoteTime,
    TradeTime,
    Exchange,
    ExchangeName,
    LastTradingDay,
    SettlementType,
    NetPercentChange,
    MarkPriceNetChange,
    MarkPricePercentChange,
    ImpliedYield,
    IsPennyPilot,
    OptionRoot,
    High52WeekPrice,
    Low52WeekPrice,
    IndicativeAskPrice,
    IndicativeBidPrice,
    IndicativeQuoteTime,
    ExerciseType,
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

/// Typed payload for a single LEVELONE_OPTIONS update.
///
/// LEVELONE_OPTIONS uses Schwab's "Change" delivery type: only the fields
/// that changed since the previous tick are present. Every numeric-indexed
/// field is therefore `Option<T>`. The `key`, `delayed`, `assetMainType`,
/// `assetSubType`, and `cusip` fields appear on every message and are not
/// numerically indexed; the remaining fields correspond 1:1 with the
/// `Field` enum above.
///
/// **Decimal precision**: prices deserialize via `rust_decimal::serde::float_option`,
/// which routes through `f64`. For Schwab option quotes this is well within
/// `f64`'s ~15-digit precision.
///
/// **Timestamps** are milliseconds since the Unix epoch (`u64`)
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
    pub description: Option<String>,
    // Field 2
    #[serde(with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    // Field 3
    #[serde(with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    // Field 4
    #[serde(with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    // Field 5
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    // Field 6
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    // Field 7
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    // Field 8
    pub total_volume: Option<u64>,
    // Field 9
    pub open_interest: Option<i64>,
    // Field 10
    #[serde(with = "decimal_opt")]
    pub volatility: Option<Decimal>,
    // Field 11
    #[serde(with = "decimal_opt")]
    pub money_intrinsic_value: Option<Decimal>,
    // Field 12
    pub expiration_year: Option<i32>,
    // Field 13
    #[serde(with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    // Field 14
    pub digits: Option<i32>,
    // Field 15
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    // Field 16
    pub bid_size: Option<u64>,
    // Field 17
    pub ask_size: Option<u64>,
    // Field 18
    pub last_size: Option<u64>,
    // Field 19
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    // Field 20
    #[serde(with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    // Field 21
    pub contract_type: Option<String>,
    // Field 22
    pub underlying: Option<String>,
    // Field 23
    pub expiration_month: Option<i32>,
    // Field 24
    pub deliverables: Option<String>,
    // Field 25
    #[serde(with = "decimal_opt")]
    pub time_value: Option<Decimal>,
    // Field 26
    pub expiration_day: Option<i32>,
    // Field 27
    pub days_to_expiration: Option<i32>,
    // Field 28
    #[serde(with = "decimal_opt")]
    pub delta: Option<Decimal>,
    // Field 29
    #[serde(with = "decimal_opt")]
    pub gamma: Option<Decimal>,
    // Field 30
    #[serde(with = "decimal_opt")]
    pub theta: Option<Decimal>,
    // Field 31
    #[serde(with = "decimal_opt")]
    pub vega: Option<Decimal>,
    // Field 32
    #[serde(with = "decimal_opt")]
    pub rho: Option<Decimal>,
    // Field 33
    pub security_status: Option<String>,
    // Field 34
    #[serde(with = "decimal_opt")]
    pub theoretical_option_value: Option<Decimal>,
    // Field 35
    #[serde(with = "decimal_opt")]
    pub underlying_price: Option<Decimal>,
    // Field 36
    pub uv_expiration_type: Option<String>,
    // Field 37
    #[serde(with = "decimal_opt")]
    pub mark_price: Option<Decimal>,
    // Field 38
    pub quote_time: Option<u64>,
    // Field 39
    pub trade_time: Option<u64>,
    // Field 40
    pub exchange: Option<String>,
    // Field 41
    pub exchange_name: Option<String>,
    // Field 42
    pub last_trading_day: Option<i64>,
    // Field 43
    pub settlement_type: Option<String>,
    // Field 44
    #[serde(with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    // Field 45
    #[serde(with = "decimal_opt")]
    pub mark_price_net_change: Option<Decimal>,
    // Field 46
    #[serde(with = "decimal_opt")]
    pub mark_price_percent_change: Option<Decimal>,
    // Field 47
    #[serde(with = "decimal_opt")]
    pub implied_yield: Option<Decimal>,
    // Field 48
    pub is_penny_pilot: Option<bool>,
    // Field 49
    pub option_root: Option<String>,
    // Field 50
    #[serde(with = "decimal_opt")]
    pub high52_week_price: Option<Decimal>,
    // Field 51
    #[serde(with = "decimal_opt")]
    pub low52_week_price: Option<Decimal>,
    // Field 52
    #[serde(with = "decimal_opt")]
    pub indicative_ask_price: Option<Decimal>,
    // Field 53
    #[serde(with = "decimal_opt")]
    pub indicative_bid_price: Option<Decimal>,
    // Field 54
    pub indicative_quote_time: Option<u64>,
    // Field 55
    pub exercise_type: Option<String>,
}

impl Content {
    /// Decode a remapped JSON object (numeric keys already resolved to
    /// snake_case names by the streamer frame parser) into a typed batch.
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "LEVELONE_OPTIONS content".to_string(),
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
            keys: vec!["AAPL  240315C00200000".to_string()],
            fields: vec![Field::Symbol, Field::BidPrice, Field::Delta, Field::Gamma],
        };
        let serialized = serde_json::to_string(&params).unwrap();
        assert_eq!(
            serialized,
            r#"{"keys":"AAPL  240315C00200000","fields":"0,2,28,29"}"#
        );
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["XYZ 251219C00050000".to_string()],
            fields: vec![Field::Symbol, Field::Delta],
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
        assert_eq!(Field::High52WeekPrice.to_string(), "high52_week_price");
        assert_eq!(Field::Low52WeekPrice.to_string(), "low52_week_price");
        assert_eq!(Field::UvExpirationType.to_string(), "uv_expiration_type");
        assert_eq!(Field::IsPennyPilot.to_string(), "is_penny_pilot");
        assert_eq!(Field::DaysToExpiration.to_string(), "days_to_expiration");
        assert_eq!(
            Field::MoneyIntrinsicValue.to_string(),
            "money_intrinsic_value"
        );
    }
}
