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
    AskId,
    BidId,
    TotalVolume,
    LastSize,
    HighPrice,
    LowPrice,
    ClosePrice,
    ExchangeId,
    Marginable,
    Description,
    LastId,
    OpenPrice,
    NetChange,
    High52WeekPrice,
    Low52WeekPrice,
    PeRatio,
    AnnualDividendAmount,
    DividendYield,
    Nav,
    ExchangeName,
    DividendDate,
    RegularMarketQuote,
    RegularMarketTrade,
    RegularMarketLastPrice,
    RegularMarketLastSize,
    RegularMarketNetChange,
    SecurityStatus,
    MarkPrice,
    QuoteTime,
    TradeTime,
    RegularMarketTradeTime,
    BidTime,
    AskTime,
    AskMicId,
    BidMicId,
    LastMicId,
    NetPercentageChange,
    RegularMarketPercentageChange,
    MarkPriceNetChange,
    MarkPricePercentageChange,
    HardToBorrowQuantity,
    HardToBorrowRate,
    HardToBorrow,
    Shortable,
    PostMarketNetChange,
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
/// **Decimal precision**: prices deserialize via `rust_decimal::serde::float_option`,
/// which routes through `f64`. For Schwab equity quotes this is well within
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
    pub ask_id: Option<String>,
    // Field 7
    pub bid_id: Option<String>,
    // Field 8
    pub total_volume: Option<u64>,
    // Field 9
    pub last_size: Option<u64>,
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
    pub exchange_id: Option<String>,
    // Field 14
    pub marginable: Option<bool>,
    // Field 15
    pub description: Option<String>,
    // Field 16
    pub last_id: Option<String>,
    // Field 17
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    // Field 18
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    // Field 19
    #[serde(with = "decimal_opt")]
    pub high52_week_price: Option<Decimal>,
    // Field 20
    #[serde(with = "decimal_opt")]
    pub low52_week_price: Option<Decimal>,
    // Field 21
    #[serde(with = "decimal_opt")]
    pub pe_ratio: Option<Decimal>,
    // Field 22
    #[serde(with = "decimal_opt")]
    pub annual_dividend_amount: Option<Decimal>,
    // Field 23
    #[serde(with = "decimal_opt")]
    pub dividend_yield: Option<Decimal>,
    // Field 24
    #[serde(with = "decimal_opt")]
    pub nav: Option<Decimal>,
    // Field 25
    pub exchange_name: Option<String>,
    // Field 26
    pub dividend_date: Option<String>,
    // Field 27
    pub regular_market_quote: Option<bool>,
    // Field 28
    pub regular_market_trade: Option<bool>,
    // Field 29
    #[serde(with = "decimal_opt")]
    pub regular_market_last_price: Option<Decimal>,
    // Field 30
    pub regular_market_last_size: Option<u64>,
    // Field 31
    #[serde(with = "decimal_opt")]
    pub regular_market_net_change: Option<Decimal>,
    // Field 32
    pub security_status: Option<String>,
    // Field 33
    #[serde(with = "decimal_opt")]
    pub mark_price: Option<Decimal>,
    // Field 34
    pub quote_time: Option<u64>,
    // Field 35
    pub trade_time: Option<u64>,
    // Field 36
    pub regular_market_trade_time: Option<u64>,
    // Field 37
    pub bid_time: Option<u64>,
    // Field 38
    pub ask_time: Option<u64>,
    // Field 39
    pub ask_mic_id: Option<String>,
    // Field 40
    pub bid_mic_id: Option<String>,
    // Field 41
    pub last_mic_id: Option<String>,
    // Field 42
    #[serde(with = "decimal_opt")]
    pub net_percentage_change: Option<Decimal>,
    // Field 43
    #[serde(with = "decimal_opt")]
    pub regular_market_percentage_change: Option<Decimal>,
    // Field 44
    #[serde(with = "decimal_opt")]
    pub mark_price_net_change: Option<Decimal>,
    // Field 45
    #[serde(with = "decimal_opt")]
    pub mark_price_percentage_change: Option<Decimal>,
    // Field 46
    pub hard_to_borrow_quantity: Option<i64>,
    // Field 47
    #[serde(with = "decimal_opt")]
    pub hard_to_borrow_rate: Option<Decimal>,
    // Field 48
    pub hard_to_borrow: Option<i8>,
    // Field 49
    pub shortable: Option<i8>,
    // Field 50
    #[serde(with = "decimal_opt")]
    pub post_market_net_change: Option<Decimal>,
    // Field 51
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
