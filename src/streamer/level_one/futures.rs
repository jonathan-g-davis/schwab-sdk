//! `LEVELONE_FUTURES` streamer service.
//!
//! Delivery type: Change. Fields not present on a tick stay `None`.
//!
//! Futures symbols are Schwab-standard: `/` + root + month code + 2-digit year
//! (e.g. `/ESZ24`).

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::{Error, Result};
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::LevelOneFutures;
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
    ExchangeId,
    Description,
    LastId,
    OpenPrice,
    NetChange,
    FuturePercentChange,
    ExchangeName,
    SecurityStatus,
    OpenInterest,
    Mark,
    Tick,
    TickAmount,
    Product,
    FuturePriceFormat,
    FutureTradingHours,
    FutureIsTradable,
    FutureMultiplier,
    FutureIsActive,
    FutureSettlementPrice,
    FutureActiveSymbol,
    FutureExpirationDate,
    ExpirationStyle,
    AskTime,
    BidTime,
    QuotedInSession,
    SettlementDate,
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

/// Typed payload for a single LEVELONE_FUTURES update.
///
/// **Decimal precision**: prices deserialize via `rust_decimal::serde::float_option`,
/// which routes through `f64` (~15-digit precision).
///
/// **Timestamps** are milliseconds since the Unix epoch (`u64`).
///
/// **`future_price_format`** is documented by Schwab as `numerator,denominator`
/// (e.g. `"3,32"` for fixed-income futures, `"D,D"` for pure-decimal equity
/// futures).
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
    // Field 6 - currently "?" since all quotes are CME.
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
    pub exchange_id: Option<String>,
    // Field 16
    pub description: Option<String>,
    // Field 17
    pub last_id: Option<String>,
    // Field 18
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    // Field 19
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    // Field 20
    #[serde(with = "decimal_opt")]
    pub future_percent_change: Option<Decimal>,
    // Field 21
    pub exchange_name: Option<String>,
    // Field 22 - Normal / Halted / Closed.
    pub security_status: Option<String>,
    // Field 23
    pub open_interest: Option<i64>,
    // Field 24 - mark-to-market value: last_price if inside spread, else midpoint.
    #[serde(with = "decimal_opt")]
    pub mark: Option<Decimal>,
    // Field 25 - minimum price increment.
    #[serde(with = "decimal_opt")]
    pub tick: Option<Decimal>,
    // Field 26 - tick * multiplier.
    #[serde(with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    // Field 27
    pub product: Option<String>,
    // Field 28 - see struct-level docs.
    pub future_price_format: Option<String>,
    // Field 29 - Schwab packs day-of-week and open/close into a string; parse on demand.
    pub future_trading_hours: Option<String>,
    // Field 30
    pub future_is_tradable: Option<bool>,
    // Field 31 - point value (e.g. 50.0 for ES).
    #[serde(with = "decimal_opt")]
    pub future_multiplier: Option<Decimal>,
    // Field 32
    pub future_is_active: Option<bool>,
    // Field 33
    #[serde(with = "decimal_opt")]
    pub future_settlement_price: Option<Decimal>,
    // Field 34
    pub future_active_symbol: Option<String>,
    // Field 35 - ms since Unix epoch.
    pub future_expiration_date: Option<i64>,
    // Field 36
    pub expiration_style: Option<String>,
    // Field 37
    pub ask_time: Option<u64>,
    // Field 38
    pub bid_time: Option<u64>,
    // Field 39
    pub quoted_in_session: Option<bool>,
    // Field 40 - ms since Unix epoch.
    pub settlement_date: Option<i64>,
}

impl Content {
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "LEVELONE_FUTURES content".to_string(),
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
    fn parses_level_one_futures_data_into_typed_content() {
        // /ESZ24 (E-Mini S&P 500 Dec 2024) with quotes, volume, multiplier.
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_FUTURES",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "/ESZ24",
                    "delayed": false,
                    "1": 5025.25, "2": 5025.50, "3": 5025.25,
                    "4": 12, "5": 9,
                    "8": 1234567, "12": 5050.00, "13": 5005.75,
                    "16": "E-Mini S&P 500 Dec 24",
                    "24": 5025.375,
                    "25": 0.25, "26": 12.50,
                    "30": true, "31": 50.0, "32": true
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneFutures);
        let DataContent::LevelOneFutures(items) = &payload.content else {
            panic!("expected LevelOneFutures, got {:?}", payload.content);
        };
        assert_eq!(items.len(), 1);
        let es = &items[0];
        assert_eq!(es.key, "/ESZ24");
        assert_eq!(es.bid_price, Some(dec!(5025.25)));
        assert_eq!(es.ask_price, Some(dec!(5025.50)));
        assert_eq!(es.last_price, Some(dec!(5025.25)));
        assert_eq!(es.bid_size, Some(12));
        assert_eq!(es.ask_size, Some(9));
        assert_eq!(es.total_volume, Some(1234567));
        assert_eq!(es.high_price, Some(dec!(5050.00)));
        assert_eq!(es.low_price, Some(dec!(5005.75)));
        assert_eq!(es.description.as_deref(), Some("E-Mini S&P 500 Dec 24"));
        assert_eq!(es.mark, Some(dec!(5025.375)));
        assert_eq!(es.tick, Some(dec!(0.25)));
        assert_eq!(es.tick_amount, Some(dec!(12.50)));
        assert_eq!(es.future_is_tradable, Some(true));
        assert_eq!(es.future_multiplier, Some(dec!(50.0)));
        assert_eq!(es.future_is_active, Some(true));
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["/ESZ24".to_string()],
            vec![Field::Symbol, Field::BidPrice, Field::Mark, Field::Tick],
        );
        assert_eq!(value["keys"], "/ESZ24");
        assert_eq!(value["fields"], "0,1,24,25");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["/ESZ24".to_string(), "/NQZ24".to_string()],
            fields: vec![Field::Symbol, Field::LastPrice],
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
        // The spec calls field 20 "Future Percent Change" - verify the
        // snake_case key matches what `transform_keys` will emit.
        assert_eq!(
            Field::FuturePercentChange.to_string(),
            "future_percent_change"
        );
        assert_eq!(Field::FutureIsTradable.to_string(), "future_is_tradable");
        assert_eq!(Field::QuotedInSession.to_string(), "quoted_in_session");
        assert_eq!(Field::FuturePriceFormat.to_string(), "future_price_format");
    }
}
