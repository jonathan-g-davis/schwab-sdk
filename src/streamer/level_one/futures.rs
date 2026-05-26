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
/// Numbered subscription field for LEVELONE_FUTURES.
///
/// Pass any combination to [`SubscribeRequest::fields`](crate::streamer::SubscribeRequest::fields);
/// each variant corresponds 1:1 with the matching field on [`Content`].
#[repr(u8)]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum Field {
    /// Wire symbol (field 0).
    Symbol,
    /// Best bid (field 1).
    BidPrice,
    /// Best ask (field 2).
    AskPrice,
    /// Last trade price (field 3).
    LastPrice,
    /// Best bid size, contracts (field 4).
    BidSize,
    /// Best ask size, contracts (field 5).
    AskSize,
    /// MIC venue id for the best bid (field 6).
    BidId,
    /// MIC venue id for the best ask (field 7).
    AskId,
    /// Cumulative session volume, contracts (field 8).
    TotalVolume,
    /// Last trade size, contracts (field 9).
    LastSize,
    /// Last quote time, epoch milliseconds (field 10).
    QuoteTime,
    /// Last trade time, epoch milliseconds (field 11).
    TradeTime,
    /// Day high (field 12).
    HighPrice,
    /// Day low (field 13).
    LowPrice,
    /// Prior session close (field 14).
    ClosePrice,
    /// Schwab exchange code (field 15).
    ExchangeId,
    /// Contract description (field 16).
    Description,
    /// MIC venue id for the last trade (field 17).
    LastId,
    /// Day open (field 18).
    OpenPrice,
    /// Net change since prior close (field 19).
    NetChange,
    /// Session price change as a fraction (field 20).
    FuturePercentChange,
    /// Exchange display name (field 21).
    ExchangeName,
    /// Security status string (field 22).
    SecurityStatus,
    /// Open interest, contracts (field 23).
    OpenInterest,
    /// Mark price (field 24).
    Mark,
    /// Minimum tick size (field 25).
    Tick,
    /// Notional value of one tick, USD (field 26).
    TickAmount,
    /// Product/root description (field 27).
    Product,
    /// Schwab price-format string (field 28).
    FuturePriceFormat,
    /// Trading-hours description (field 29).
    FutureTradingHours,
    /// `true` if the contract is tradable (field 30).
    FutureIsTradable,
    /// Contract multiplier, USD per point (field 31).
    FutureMultiplier,
    /// `true` if this contract is the front month (field 32).
    FutureIsActive,
    /// Settlement price, USD (field 33).
    FutureSettlementPrice,
    /// Active (front-month) symbol for this product (field 34).
    FutureActiveSymbol,
    /// Expiration date, epoch milliseconds (field 35).
    FutureExpirationDate,
    /// Expiration style description (field 36).
    ExpirationStyle,
    /// Last ask time, epoch milliseconds (field 37).
    AskTime,
    /// Last bid time, epoch milliseconds (field 38).
    BidTime,
    /// `true` if the quote was sampled during a regular session (field 39).
    QuotedInSession,
    /// Settlement date, epoch milliseconds (field 40).
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
/// **Timestamps** are milliseconds since the Unix epoch (`u64`).
///
/// **`future_price_format`** is documented by Schwab as `numerator,denominator`
/// (e.g. `"3,32"` for fixed-income futures, `"D,D"` for pure-decimal equity
/// futures).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the futures symbol).
    pub key: String,
    /// `true` if the quote is delayed.
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
    /// Field 1: best bid.
    #[serde(with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Field 2: best ask.
    #[serde(with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Field 3: last trade price.
    #[serde(with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Field 4: best bid size, contracts.
    pub bid_size: Option<u64>,
    /// Field 5: best ask size, contracts.
    pub ask_size: Option<u64>,
    /// Field 6: MIC venue id for the best bid. Typically `"?"` since all
    /// quotes are CME.
    pub bid_id: Option<String>,
    /// Field 7: MIC venue id for the best ask.
    pub ask_id: Option<String>,
    /// Field 8: cumulative session volume, contracts.
    pub total_volume: Option<u64>,
    /// Field 9: last trade size, contracts.
    pub last_size: Option<u64>,
    /// Field 10: last quote time, epoch milliseconds.
    pub quote_time: Option<u64>,
    /// Field 11: last trade time, epoch milliseconds.
    pub trade_time: Option<u64>,
    /// Field 12: day high.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Field 13: day low.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Field 14: prior session close.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Field 15: Schwab exchange code.
    pub exchange_id: Option<String>,
    /// Field 16: contract description.
    pub description: Option<String>,
    /// Field 17: MIC venue id for the last trade.
    pub last_id: Option<String>,
    /// Field 18: day open.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Field 19: net change since prior close.
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Field 20: session price change as a fraction.
    #[serde(with = "decimal_opt")]
    pub future_percent_change: Option<Decimal>,
    /// Field 21: exchange display name.
    pub exchange_name: Option<String>,
    /// Field 22: security status string (Normal / Halted / Closed).
    pub security_status: Option<String>,
    /// Field 23: open interest, contracts.
    pub open_interest: Option<i64>,
    /// Field 24: mark-to-market value (last_price if inside the spread,
    /// else midpoint).
    #[serde(with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Field 25: minimum price increment.
    #[serde(with = "decimal_opt")]
    pub tick: Option<Decimal>,
    /// Field 26: notional value of one tick (`tick * multiplier`), USD.
    #[serde(with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    /// Field 27: product/root description.
    pub product: Option<String>,
    /// Field 28: price-format string (see struct-level docs).
    pub future_price_format: Option<String>,
    /// Field 29: trading-hours description. Schwab packs day-of-week and
    /// open/close into a single string; parse on demand.
    pub future_trading_hours: Option<String>,
    /// Field 30: `true` if the contract is tradable.
    pub future_is_tradable: Option<bool>,
    /// Field 31: contract multiplier - point value, e.g. 50.0 for ES.
    #[serde(with = "decimal_opt")]
    pub future_multiplier: Option<Decimal>,
    /// Field 32: `true` if this contract is the front month.
    pub future_is_active: Option<bool>,
    /// Field 33: settlement price, USD.
    #[serde(with = "decimal_opt")]
    pub future_settlement_price: Option<Decimal>,
    /// Field 34: active (front-month) symbol for this product.
    pub future_active_symbol: Option<String>,
    /// Field 35: expiration date, epoch milliseconds.
    pub future_expiration_date: Option<i64>,
    /// Field 36: expiration style description.
    pub expiration_style: Option<String>,
    /// Field 37: last ask time, epoch milliseconds.
    pub ask_time: Option<u64>,
    /// Field 38: last bid time, epoch milliseconds.
    pub bid_time: Option<u64>,
    /// Field 39: `true` if the quote was sampled during a regular session.
    pub quoted_in_session: Option<bool>,
    /// Field 40: settlement date, epoch milliseconds.
    pub settlement_date: Option<i64>,
}

impl Content {
    /// Decode a remapped JSON object (numeric keys already resolved to
    /// snake_case names by the streamer frame parser) into a typed batch.
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
