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
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::LevelOneForex;
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
/// Numbered subscription field for LEVELONE_FOREX.
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
    /// Best bid size (field 4).
    BidSize,
    /// Best ask size (field 5).
    AskSize,
    /// Cumulative session volume (field 6).
    TotalVolume,
    /// Last trade size (field 7).
    LastSize,
    /// Last quote time, epoch milliseconds (field 8).
    QuoteTime,
    /// Last trade time, epoch milliseconds (field 9).
    TradeTime,
    /// Day high (field 10).
    HighPrice,
    /// Day low (field 11).
    LowPrice,
    /// Prior session close (field 12).
    ClosePrice,
    /// Schwab exchange code (field 13).
    Exchange,
    /// Pair description (field 14).
    Description,
    /// Day open (field 15).
    OpenPrice,
    /// Net change since prior close (field 16).
    NetChange,
    /// Net change since prior close as a fraction (field 17).
    PercentChange,
    /// Exchange display name (field 18).
    ExchangeName,
    /// Decimal digits Schwab uses for price display (field 19).
    Digits,
    /// Security status string (field 20).
    SecurityStatus,
    /// Minimum tick size (field 21).
    Tick,
    /// Notional value of one tick (field 22).
    TickAmount,
    /// Product/pair category (field 23).
    Product,
    /// Trading-hours description (field 24).
    TradingHours,
    /// `true` if the pair is tradable (field 25).
    IsTradable,
    /// Market maker name, when applicable (field 26).
    MarketMaker,
    /// 52-week high (field 27).
    High52Week,
    /// 52-week low (field 28).
    Low52Week,
    /// Mark-to-market value (field 29).
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
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the forex pair, e.g. `"EUR/USD"`).
    pub key: String,
    /// `true` if the quote is delayed.
    pub delayed: bool,
    /// Asset class string (`"FOREX"`).
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
    /// Field 4: best bid size.
    pub bid_size: Option<u64>,
    /// Field 5: best ask size.
    pub ask_size: Option<u64>,
    /// Field 6: cumulative session volume.
    pub total_volume: Option<u64>,
    /// Field 7: last trade size.
    pub last_size: Option<u64>,
    /// Field 8: last quote time, epoch milliseconds.
    pub quote_time: Option<u64>,
    /// Field 9: last trade time, epoch milliseconds.
    pub trade_time: Option<u64>,
    /// Field 10: day high.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Field 11: day low.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Field 12: prior session close.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Field 13: Schwab exchange code.
    pub exchange: Option<String>,
    /// Field 14: pair description.
    pub description: Option<String>,
    /// Field 15: day open.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Field 16: net change since prior close.
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Field 17: net change since prior close as a fraction
    /// (`if close > 0: (last - close) / close, else 0`).
    #[serde(with = "decimal_opt")]
    pub percent_change: Option<Decimal>,
    /// Field 18: exchange display name.
    pub exchange_name: Option<String>,
    /// Field 19: decimal digits Schwab uses for price display.
    pub digits: Option<i32>,
    /// Field 20: security status string.
    pub security_status: Option<String>,
    /// Field 21: minimum tick size.
    #[serde(with = "decimal_opt")]
    pub tick: Option<Decimal>,
    /// Field 22: notional value of one tick.
    #[serde(with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    /// Field 23: product/pair category.
    pub product: Option<String>,
    /// Field 24: trading-hours description.
    pub trading_hours: Option<String>,
    /// Field 25: `true` if the pair is tradable through Schwab.
    pub is_tradable: Option<bool>,
    /// Field 26: market maker name, when applicable.
    pub market_maker: Option<String>,
    /// Field 27: 52-week high.
    #[serde(with = "decimal_opt")]
    pub high52_week: Option<Decimal>,
    /// Field 28: 52-week low.
    #[serde(with = "decimal_opt")]
    pub low52_week: Option<Decimal>,
    /// Field 29: mark-to-market value.
    #[serde(with = "decimal_opt")]
    pub mark: Option<Decimal>,
}

impl Content {
    /// Decode a remapped JSON object (numeric keys already resolved to
    /// snake_case names by the streamer frame parser) into a typed batch.
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
    use crate::streamer::StreamerRequest;
    use crate::streamer::StreamerResponse;
    use crate::streamer::response::{DataContent, parse};
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};
    use rust_decimal_macros::dec;

    #[test]
    fn parses_level_one_forex_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_FOREX",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "EUR/USD",
                    "delayed": false,
                    "1": 1.0825, "2": 1.0826, "3": 1.08255,
                    "4": 1000000, "5": 1500000,
                    "10": 1.0850, "11": 1.0810, "12": 1.0820,
                    "14": "Euro/US Dollar",
                    "16": 0.00055, "17": 0.0508,
                    "19": 5,
                    "25": true, "29": 1.08255
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneForex);
        let DataContent::LevelOneForex(items) = &payload.content else {
            panic!("expected LevelOneForex");
        };
        let eur = &items[0];
        assert_eq!(eur.key, "EUR/USD");
        assert_eq!(eur.bid_price, Some(dec!(1.0825)));
        assert_eq!(eur.ask_price, Some(dec!(1.0826)));
        assert_eq!(eur.last_price, Some(dec!(1.08255)));
        assert_eq!(eur.bid_size, Some(1_000_000));
        assert_eq!(eur.ask_size, Some(1_500_000));
        assert_eq!(eur.description.as_deref(), Some("Euro/US Dollar"));
        assert_eq!(eur.percent_change, Some(dec!(0.0508)));
        assert_eq!(eur.digits, Some(5));
        assert_eq!(eur.is_tradable, Some(true));
        assert_eq!(eur.mark, Some(dec!(1.08255)));
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["EUR/USD".to_string()],
            vec![Field::Symbol, Field::BidPrice, Field::AskPrice, Field::Mark],
        );
        assert_eq!(value["keys"], "EUR/USD");
        assert_eq!(value["fields"], "0,1,2,29");
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
