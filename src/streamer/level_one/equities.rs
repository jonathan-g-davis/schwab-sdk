//! `LEVELONE_EQUITIES` streamer service.
//!
//! Delivery type: Change. Fields not present on a tick stay `None`.

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::{Error, Result};
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::LevelOneEquities;
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
/// Numbered subscription field for LEVELONE_EQUITIES.
///
/// Pass any combination to [`SubscribeRequest::fields`](crate::streamer::SubscribeRequest::fields);
/// each variant corresponds 1:1 with the matching field on [`Content`].
#[repr(u8)]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum Field {
    /// Wire symbol (field 0).
    Symbol,
    /// Best bid, USD (field 1).
    BidPrice,
    /// Best ask, USD (field 2).
    AskPrice,
    /// Last trade price, USD (field 3).
    LastPrice,
    /// Best bid size (field 4).
    BidSize,
    /// Best ask size (field 5).
    AskSize,
    /// MIC venue id for the best ask (field 6).
    AskId,
    /// MIC venue id for the best bid (field 7).
    BidId,
    /// Cumulative session volume (field 8).
    TotalVolume,
    /// Last trade size (field 9).
    LastSize,
    /// Day high, USD (field 10).
    HighPrice,
    /// Day low, USD (field 11).
    LowPrice,
    /// Prior session close, USD (field 12).
    ClosePrice,
    /// Schwab exchange code (field 13).
    ExchangeId,
    /// `true` if the security is marginable (field 14).
    Marginable,
    /// Human-readable description (field 15).
    Description,
    /// MIC venue id for the last trade (field 16).
    LastId,
    /// Day open, USD (field 17).
    OpenPrice,
    /// Net change since prior close, USD (field 18).
    NetChange,
    /// 52-week high, USD (field 19).
    High52WeekPrice,
    /// 52-week low, USD (field 20).
    Low52WeekPrice,
    /// P/E ratio (field 21).
    PeRatio,
    /// Annual dividend amount, USD per share (field 22).
    AnnualDividendAmount,
    /// Trailing dividend yield as a fraction (field 23).
    DividendYield,
    /// Net asset value for funds (field 24).
    Nav,
    /// Exchange display name (field 25).
    ExchangeName,
    /// Dividend date string (field 26).
    DividendDate,
    /// `true` if a regular-session quote is available (field 27).
    RegularMarketQuote,
    /// `true` if a regular-session trade has occurred (field 28).
    RegularMarketTrade,
    /// Last regular-session trade price, USD (field 29).
    RegularMarketLastPrice,
    /// Last regular-session trade size (field 30).
    RegularMarketLastSize,
    /// Regular-session net change, USD (field 31).
    RegularMarketNetChange,
    /// Security status string (field 32).
    SecurityStatus,
    /// Mark price, USD (field 33).
    MarkPrice,
    /// Last quote time, epoch milliseconds (field 34).
    QuoteTime,
    /// Last trade time, epoch milliseconds (field 35).
    TradeTime,
    /// Last regular-session trade time, epoch milliseconds (field 36).
    RegularMarketTradeTime,
    /// Last bid time, epoch milliseconds (field 37).
    BidTime,
    /// Last ask time, epoch milliseconds (field 38).
    AskTime,
    /// MIC venue id for the best ask (field 39).
    AskMicId,
    /// MIC venue id for the best bid (field 40).
    BidMicId,
    /// MIC venue id for the last trade (field 41).
    LastMicId,
    /// Net change since prior close as a fraction (field 42).
    NetPercentageChange,
    /// Regular-session change as a fraction (field 43).
    RegularMarketPercentageChange,
    /// Mark change since prior close, USD (field 44).
    MarkPriceNetChange,
    /// Mark change since prior close as a fraction (field 45).
    MarkPricePercentageChange,
    /// Hard-to-borrow available quantity (field 46).
    HardToBorrowQuantity,
    /// Hard-to-borrow annualized rate (field 47).
    HardToBorrowRate,
    /// Hard-to-borrow flag (field 48).
    HardToBorrow,
    /// Shortable flag (field 49).
    Shortable,
    /// Post-market net change, USD (field 50).
    PostMarketNetChange,
    /// Post-market change as a fraction (field 51).
    PostMarketPercentageChange,
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

/// Typed payload for a single LEVELONE_EQUITIES update.
///
/// LEVELONE_EQUITIES uses Schwab's "Change" delivery type: only the fields
/// that changed since the previous tick are present. Every numeric-indexed
/// field is therefore `Option<T>`. The `key`, `delayed`, `assetMainType`,
/// `assetSubType`, and `cusip` fields appear on every message and are not
/// numerically indexed; the remaining fields correspond 1:1 with the
/// `Field` enum above.
///
/// **Timestamps** are milliseconds since the Unix epoch (`u64`).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the symbol the update is for).
    pub key: String,
    /// `true` if the quote is delayed.
    pub delayed: bool,
    /// Asset class string (`"EQUITY"` for this service).
    #[serde(rename = "assetMainType")]
    pub asset_main_type: Option<String>,
    /// Asset sub-type string (e.g. `"COE"`, `"ETF"`).
    #[serde(rename = "assetSubType")]
    pub asset_sub_type: Option<String>,
    /// CUSIP.
    pub cusip: Option<String>,

    /// Field 0: wire symbol.
    pub symbol: Option<String>,
    /// Field 1: best bid, USD.
    #[serde(with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Field 2: best ask, USD.
    #[serde(with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Field 3: last trade price, USD.
    #[serde(with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Field 4: best bid size.
    pub bid_size: Option<u64>,
    /// Field 5: best ask size.
    pub ask_size: Option<u64>,
    /// Field 6: MIC venue id for the best ask.
    pub ask_id: Option<String>,
    /// Field 7: MIC venue id for the best bid.
    pub bid_id: Option<String>,
    /// Field 8: cumulative session volume.
    pub total_volume: Option<u64>,
    /// Field 9: last trade size.
    pub last_size: Option<u64>,
    /// Field 10: day high, USD.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Field 11: day low, USD.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Field 12: prior session close, USD.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Field 13: Schwab exchange code.
    pub exchange_id: Option<String>,
    /// Field 14: `true` if the security is marginable.
    pub marginable: Option<bool>,
    /// Field 15: human-readable description.
    pub description: Option<String>,
    /// Field 16: MIC venue id for the last trade.
    pub last_id: Option<String>,
    /// Field 17: day open, USD.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Field 18: net change since prior close, USD.
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Field 19: 52-week high, USD.
    #[serde(with = "decimal_opt")]
    pub high52_week_price: Option<Decimal>,
    /// Field 20: 52-week low, USD.
    #[serde(with = "decimal_opt")]
    pub low52_week_price: Option<Decimal>,
    /// Field 21: P/E ratio.
    #[serde(with = "decimal_opt")]
    pub pe_ratio: Option<Decimal>,
    /// Field 22: annual dividend amount, USD per share.
    #[serde(with = "decimal_opt")]
    pub annual_dividend_amount: Option<Decimal>,
    /// Field 23: trailing dividend yield as a fraction.
    #[serde(with = "decimal_opt")]
    pub dividend_yield: Option<Decimal>,
    /// Field 24: net asset value for funds, USD.
    #[serde(with = "decimal_opt")]
    pub nav: Option<Decimal>,
    /// Field 25: exchange display name.
    pub exchange_name: Option<String>,
    /// Field 26: dividend date string.
    pub dividend_date: Option<String>,
    /// Field 27: `true` if a regular-session quote is available.
    pub regular_market_quote: Option<bool>,
    /// Field 28: `true` if a regular-session trade has occurred.
    pub regular_market_trade: Option<bool>,
    /// Field 29: last regular-session trade price, USD.
    #[serde(with = "decimal_opt")]
    pub regular_market_last_price: Option<Decimal>,
    /// Field 30: last regular-session trade size.
    pub regular_market_last_size: Option<u64>,
    /// Field 31: regular-session net change, USD.
    #[serde(with = "decimal_opt")]
    pub regular_market_net_change: Option<Decimal>,
    /// Field 32: security status string.
    pub security_status: Option<String>,
    /// Field 33: mark price, USD.
    #[serde(with = "decimal_opt")]
    pub mark_price: Option<Decimal>,
    /// Field 34: last quote time, epoch milliseconds.
    pub quote_time: Option<u64>,
    /// Field 35: last trade time, epoch milliseconds.
    pub trade_time: Option<u64>,
    /// Field 36: last regular-session trade time, epoch milliseconds.
    pub regular_market_trade_time: Option<u64>,
    /// Field 37: last bid time, epoch milliseconds.
    pub bid_time: Option<u64>,
    /// Field 38: last ask time, epoch milliseconds.
    pub ask_time: Option<u64>,
    /// Field 39: MIC venue id for the best ask.
    pub ask_mic_id: Option<String>,
    /// Field 40: MIC venue id for the best bid.
    pub bid_mic_id: Option<String>,
    /// Field 41: MIC venue id for the last trade.
    pub last_mic_id: Option<String>,
    /// Field 42: net change since prior close as a fraction.
    #[serde(with = "decimal_opt")]
    pub net_percentage_change: Option<Decimal>,
    /// Field 43: regular-session change as a fraction.
    #[serde(with = "decimal_opt")]
    pub regular_market_percentage_change: Option<Decimal>,
    /// Field 44: mark change since prior close, USD.
    #[serde(with = "decimal_opt")]
    pub mark_price_net_change: Option<Decimal>,
    /// Field 45: mark change since prior close as a fraction.
    #[serde(with = "decimal_opt")]
    pub mark_price_percentage_change: Option<Decimal>,
    /// Field 46: hard-to-borrow available quantity.
    pub hard_to_borrow_quantity: Option<i64>,
    /// Field 47: hard-to-borrow annualized rate.
    #[serde(with = "decimal_opt")]
    pub hard_to_borrow_rate: Option<Decimal>,
    /// Field 48: hard-to-borrow flag (Schwab uses a small-int sentinel).
    pub hard_to_borrow: Option<i8>,
    /// Field 49: shortable flag (Schwab uses a small-int sentinel).
    pub shortable: Option<i8>,
    /// Field 50: post-market net change, USD.
    #[serde(with = "decimal_opt")]
    pub post_market_net_change: Option<Decimal>,
    /// Field 51: post-market change as a fraction.
    #[serde(with = "decimal_opt")]
    pub post_market_percentage_change: Option<Decimal>,
}

impl Content {
    /// Decode a remapped JSON object (numeric keys already resolved to
    /// snake_case names by the streamer frame parser) into a typed batch.
    pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<Self>> {
        serde_json::from_value(remapped).map_err(|e| Error::Codec {
            context: "LEVELONE_EQUITIES content".to_string(),
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streamer::StreamerRequest;
    use crate::streamer::response::{DataContent, parse};
    use crate::streamer::{StreamerResponse, SubscriptionCommand};
    use rust_decimal_macros::dec;

    #[test]
    fn parses_level_one_equities_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_EQUITIES",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [
                    {
                        "key": "SCHW",
                        "delayed": false,
                        "assetMainType": "EQUITY",
                        "assetSubType": "COE",
                        "cusip": "808513105",
                        "1": 76.08, "2": 76.49, "3": 76.44,
                        "4": 3, "5": 1, "8": 5414735, "10": 76.47
                    },
                    {
                        "key": "AAPL",
                        "delayed": false,
                        "assetMainType": "EQUITY",
                        "assetSubType": "COE",
                        "cusip": "037833100",
                        "1": 183.75, "2": 183.8, "3": 183.8,
                        "4": 1, "5": 2, "8": 163224109, "10": 187
                    }
                ]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        assert_eq!(data.len(), 1);
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneEquities);
        assert_eq!(payload.timestamp, 1714949592301);
        assert_eq!(payload.command, SubscriptionCommand::Subscribe);

        let DataContent::LevelOneEquities(items) = &payload.content else {
            panic!("expected LevelOneEquities, got {:?}", payload.content);
        };
        assert_eq!(items.len(), 2);

        let schw = &items[0];
        assert_eq!(schw.key, "SCHW");
        assert!(!schw.delayed);
        assert_eq!(schw.cusip.as_deref(), Some("808513105"));
        assert_eq!(schw.bid_price, Some(dec!(76.08)));
        assert_eq!(schw.ask_price, Some(dec!(76.49)));
        assert_eq!(schw.last_price, Some(dec!(76.44)));
        assert_eq!(schw.bid_size, Some(3));
        assert_eq!(schw.ask_size, Some(1));
        assert_eq!(schw.total_volume, Some(5414735));
        assert_eq!(schw.high_price, Some(dec!(76.47)));
        // Fields not present on the wire stay None.
        assert_eq!(schw.low_price, None);
        assert_eq!(schw.dividend_yield, None);

        let aapl = &items[1];
        assert_eq!(aapl.key, "AAPL");
        assert_eq!(aapl.bid_price, Some(dec!(183.75)));
        assert_eq!(aapl.last_price, Some(dec!(183.8)));
    }

    #[test]
    fn test_serialize_parameters() {
        use crate::streamer::subscription::subscribe_parameters;

        let value = subscribe_parameters(
            vec!["AAPL".to_string()],
            vec![Field::Symbol, Field::BidPrice, Field::AskPrice],
        );
        assert_eq!(value["keys"], "AAPL");
        assert_eq!(value["fields"], "0,1,2");
    }

    #[test]
    fn from_subscription_never_panics() {
        use crate::streamer::subscription::{Command, Subscription};

        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["AAPL".to_string(), "MSFT,with,commas".to_string()],
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
}
