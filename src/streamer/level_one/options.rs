//! `LEVELONE_OPTIONS` streamer service.
//!
//! Delivery type: Change. Fields not present on a tick stay `None`.

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::{Error, Result};
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::LevelOneOptions;
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
/// Numbered subscription field for LEVELONE_OPTIONS.
///
/// Pass any combination to [`SubscribeRequest::fields`](crate::streamer::SubscribeRequest::fields);
/// each variant corresponds 1:1 with the matching field on [`Content`].
#[repr(u8)]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum Field {
    /// OSI option symbol (field 0).
    Symbol,
    /// Human-readable contract description (field 1).
    Description,
    /// Best bid premium, USD (field 2).
    BidPrice,
    /// Best ask premium, USD (field 3).
    AskPrice,
    /// Last trade premium, USD (field 4).
    LastPrice,
    /// Day high premium, USD (field 5).
    HighPrice,
    /// Day low premium, USD (field 6).
    LowPrice,
    /// Prior session close premium, USD (field 7).
    ClosePrice,
    /// Cumulative session volume, contracts (field 8).
    TotalVolume,
    /// Open interest, contracts (field 9).
    OpenInterest,
    /// Implied volatility as a percentage (field 10).
    Volatility,
    /// In-the-money portion of the premium, USD (field 11).
    MoneyIntrinsicValue,
    /// Year of expiration (field 12).
    ExpirationYear,
    /// Shares-per-contract multiplier (field 13).
    Multiplier,
    /// Number of decimal digits Schwab uses for price display (field 14).
    Digits,
    /// Day open premium, USD (field 15).
    OpenPrice,
    /// Best bid size, contracts (field 16).
    BidSize,
    /// Best ask size, contracts (field 17).
    AskSize,
    /// Last trade size, contracts (field 18).
    LastSize,
    /// Net change since prior close, USD (field 19).
    NetChange,
    /// Strike price, USD (field 20).
    StrikePrice,
    /// Put/call discriminator (`"P"`/`"C"`) (field 21).
    ContractType,
    /// Underlying symbol (field 22).
    Underlying,
    /// Month of expiration (1-12) (field 23).
    ExpirationMonth,
    /// Deliverables description (field 24).
    Deliverables,
    /// Extrinsic (time) value, USD (field 25).
    TimeValue,
    /// Day-of-month of expiration (field 26).
    ExpirationDay,
    /// Calendar days until expiration (field 27).
    DaysToExpiration,
    /// Delta (Black-Scholes) (field 28).
    Delta,
    /// Gamma (Black-Scholes) (field 29).
    Gamma,
    /// Theta (Black-Scholes) (field 30).
    Theta,
    /// Vega (Black-Scholes) (field 31).
    Vega,
    /// Rho (Black-Scholes) (field 32).
    Rho,
    /// Security status string (field 33).
    SecurityStatus,
    /// Theoretical fair value from Schwab's model, USD (field 34).
    TheoreticalOptionValue,
    /// Underlying price used in the pricing model, USD (field 35).
    UnderlyingPrice,
    /// Underlying-vehicle expiration-type code (field 36).
    UvExpirationType,
    /// Mark price, USD (field 37).
    MarkPrice,
    /// Last quote time, epoch milliseconds (field 38).
    QuoteTime,
    /// Last trade time, epoch milliseconds (field 39).
    TradeTime,
    /// Schwab exchange code (field 40).
    Exchange,
    /// Exchange display name (field 41).
    ExchangeName,
    /// Last trading day, epoch milliseconds (field 42).
    LastTradingDay,
    /// AM/PM settlement code (field 43).
    SettlementType,
    /// Net change since prior close as a fraction (field 44).
    NetPercentChange,
    /// Mark change since prior close, USD (field 45).
    MarkPriceNetChange,
    /// Mark change since prior close as a fraction (field 46).
    MarkPricePercentChange,
    /// Implied yield (where Schwab supplies one) (field 47).
    ImpliedYield,
    /// `true` if the contract is in the SEC Penny Pilot program (field 48).
    IsPennyPilot,
    /// Option root symbol (field 49).
    OptionRoot,
    /// 52-week high premium, USD (field 50).
    High52WeekPrice,
    /// 52-week low premium, USD (field 51).
    Low52WeekPrice,
    /// Indicative ask price (indicative symbols only) (field 52).
    IndicativeAskPrice,
    /// Indicative bid price (indicative symbols only) (field 53).
    IndicativeBidPrice,
    /// Indicative quote time, epoch milliseconds (field 54).
    IndicativeQuoteTime,
    /// Exercise-style code (American / European) (field 55).
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
/// **Timestamps** are milliseconds since the Unix epoch (`u64`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the OSI option symbol).
    pub key: String,
    /// `true` if the quote is delayed.
    pub delayed: bool,
    /// Asset class string (`"OPTION"`).
    #[serde(rename = "assetMainType")]
    pub asset_main_type: Option<String>,
    /// Asset sub-type string.
    #[serde(rename = "assetSubType")]
    pub asset_sub_type: Option<String>,
    /// CUSIP of the contract.
    pub cusip: Option<String>,

    /// Field 0: OSI option symbol.
    pub symbol: Option<String>,
    /// Field 1: human-readable contract description.
    pub description: Option<String>,
    /// Field 2: best bid premium, USD.
    #[serde(with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Field 3: best ask premium, USD.
    #[serde(with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Field 4: last trade premium, USD.
    #[serde(with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Field 5: day high premium, USD.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Field 6: day low premium, USD.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Field 7: prior session close premium, USD.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Field 8: cumulative session volume, contracts.
    pub total_volume: Option<u64>,
    /// Field 9: open interest, contracts.
    pub open_interest: Option<i64>,
    /// Field 10: implied volatility as a percentage.
    #[serde(with = "decimal_opt")]
    pub volatility: Option<Decimal>,
    /// Field 11: in-the-money portion of the premium, USD.
    #[serde(with = "decimal_opt")]
    pub money_intrinsic_value: Option<Decimal>,
    /// Field 12: year of expiration.
    pub expiration_year: Option<i32>,
    /// Field 13: shares-per-contract multiplier.
    #[serde(with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    /// Field 14: decimal digits Schwab uses for price display.
    pub digits: Option<i32>,
    /// Field 15: day open premium, USD.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Field 16: best bid size, contracts.
    pub bid_size: Option<u64>,
    /// Field 17: best ask size, contracts.
    pub ask_size: Option<u64>,
    /// Field 18: last trade size, contracts.
    pub last_size: Option<u64>,
    /// Field 19: net change since prior close, USD.
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Field 20: strike price, USD.
    #[serde(with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    /// Field 21: put / call discriminator (`"P"` / `"C"`).
    pub contract_type: Option<String>,
    /// Field 22: underlying symbol.
    pub underlying: Option<String>,
    /// Field 23: month of expiration (1-12).
    pub expiration_month: Option<i32>,
    /// Field 24: deliverables description.
    pub deliverables: Option<String>,
    /// Field 25: extrinsic (time) value, USD.
    #[serde(with = "decimal_opt")]
    pub time_value: Option<Decimal>,
    /// Field 26: day-of-month of expiration.
    pub expiration_day: Option<i32>,
    /// Field 27: calendar days until expiration.
    pub days_to_expiration: Option<i32>,
    /// Field 28: delta (Black-Scholes).
    #[serde(with = "decimal_opt")]
    pub delta: Option<Decimal>,
    /// Field 29: gamma (Black-Scholes).
    #[serde(with = "decimal_opt")]
    pub gamma: Option<Decimal>,
    /// Field 30: theta (Black-Scholes).
    #[serde(with = "decimal_opt")]
    pub theta: Option<Decimal>,
    /// Field 31: vega (Black-Scholes).
    #[serde(with = "decimal_opt")]
    pub vega: Option<Decimal>,
    /// Field 32: rho (Black-Scholes).
    #[serde(with = "decimal_opt")]
    pub rho: Option<Decimal>,
    /// Field 33: security status string.
    pub security_status: Option<String>,
    /// Field 34: theoretical fair value from Schwab's model, USD.
    #[serde(with = "decimal_opt")]
    pub theoretical_option_value: Option<Decimal>,
    /// Field 35: underlying price used in the pricing model, USD.
    #[serde(with = "decimal_opt")]
    pub underlying_price: Option<Decimal>,
    /// Field 36: underlying-vehicle expiration-type code.
    pub uv_expiration_type: Option<String>,
    /// Field 37: mark price, USD.
    #[serde(with = "decimal_opt")]
    pub mark_price: Option<Decimal>,
    /// Field 38: last quote time, epoch milliseconds.
    pub quote_time: Option<u64>,
    /// Field 39: last trade time, epoch milliseconds.
    pub trade_time: Option<u64>,
    /// Field 40: Schwab exchange code.
    pub exchange: Option<String>,
    /// Field 41: exchange display name.
    pub exchange_name: Option<String>,
    /// Field 42: last trading day, epoch milliseconds.
    pub last_trading_day: Option<i64>,
    /// Field 43: AM / PM settlement code.
    pub settlement_type: Option<String>,
    /// Field 44: net change since prior close as a fraction.
    #[serde(with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Field 45: mark change since prior close, USD.
    #[serde(with = "decimal_opt")]
    pub mark_price_net_change: Option<Decimal>,
    /// Field 46: mark change since prior close as a fraction.
    #[serde(with = "decimal_opt")]
    pub mark_price_percent_change: Option<Decimal>,
    /// Field 47: implied yield.
    #[serde(with = "decimal_opt")]
    pub implied_yield: Option<Decimal>,
    /// Field 48: `true` if the contract is in the SEC Penny Pilot program.
    pub is_penny_pilot: Option<bool>,
    /// Field 49: option root symbol.
    pub option_root: Option<String>,
    /// Field 50: 52-week high premium, USD.
    #[serde(with = "decimal_opt")]
    pub high52_week_price: Option<Decimal>,
    /// Field 51: 52-week low premium, USD.
    #[serde(with = "decimal_opt")]
    pub low52_week_price: Option<Decimal>,
    /// Field 52: indicative ask price (indicative symbols only).
    #[serde(with = "decimal_opt")]
    pub indicative_ask_price: Option<Decimal>,
    /// Field 53: indicative bid price (indicative symbols only).
    #[serde(with = "decimal_opt")]
    pub indicative_bid_price: Option<Decimal>,
    /// Field 54: indicative quote time, epoch milliseconds.
    pub indicative_quote_time: Option<u64>,
    /// Field 55: exercise-style code (American / European).
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
    use crate::streamer::StreamerRequest;
    use crate::streamer::StreamerResponse;
    use crate::streamer::response::{DataContent, parse};
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};
    use rust_decimal_macros::dec;

    #[test]
    fn parses_level_one_options_data_into_typed_content() {
        // An ATM-ish AAPL call: bid 5.10 / ask 5.20, last 5.15, delta 0.52,
        // gamma 0.04, theta -0.08, vega 0.13, 7 DTE.
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_OPTIONS",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL  240315C00200000",
                    "delayed": false,
                    "assetMainType": "OPTION",
                    "2": 5.10, "3": 5.20, "4": 5.15,
                    "8": 12345, "9": 6789,
                    "20": 200.0, "21": "C", "22": "AAPL",
                    "27": 7, "28": 0.52, "29": 0.04, "30": -0.08, "31": 0.13,
                    "37": 5.15,
                    "48": true
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneOptions);

        let DataContent::LevelOneOptions(items) = &payload.content else {
            panic!("expected LevelOneOptions, got {:?}", payload.content);
        };
        assert_eq!(items.len(), 1);
        let aapl = &items[0];
        assert_eq!(aapl.key, "AAPL  240315C00200000");
        assert_eq!(aapl.bid_price, Some(dec!(5.10)));
        assert_eq!(aapl.ask_price, Some(dec!(5.20)));
        assert_eq!(aapl.last_price, Some(dec!(5.15)));
        assert_eq!(aapl.total_volume, Some(12345));
        assert_eq!(aapl.open_interest, Some(6789));
        assert_eq!(aapl.strike_price, Some(dec!(200.0)));
        assert_eq!(aapl.contract_type.as_deref(), Some("C"));
        assert_eq!(aapl.underlying.as_deref(), Some("AAPL"));
        assert_eq!(aapl.days_to_expiration, Some(7));
        assert_eq!(aapl.delta, Some(dec!(0.52)));
        assert_eq!(aapl.gamma, Some(dec!(0.04)));
        assert_eq!(aapl.theta, Some(dec!(-0.08)));
        assert_eq!(aapl.vega, Some(dec!(0.13)));
        assert_eq!(aapl.mark_price, Some(dec!(5.15)));
        assert_eq!(aapl.is_penny_pilot, Some(true));
        // Fields not on wire stay None.
        assert_eq!(aapl.rho, None);
        assert_eq!(aapl.implied_yield, None);
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["AAPL  240315C00200000".to_string()],
            vec![Field::Symbol, Field::BidPrice, Field::Delta, Field::Gamma],
        );
        assert_eq!(value["keys"], "AAPL  240315C00200000");
        assert_eq!(value["fields"], "0,2,28,29");
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
