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
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::LevelOneFuturesOptions;
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
/// Numbered subscription field for LEVELONE_FUTURES_OPTIONS.
///
/// Pass any combination to [`SubscribeRequest::fields`](crate::streamer::SubscribeRequest::fields);
/// each variant corresponds 1:1 with the matching field on [`Content`].
#[repr(u8)]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum Field {
    /// Wire symbol (field 0).
    Symbol,
    /// Best bid premium (field 1).
    BidPrice,
    /// Best ask premium (field 2).
    AskPrice,
    /// Last trade premium (field 3).
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
    /// Day high premium (field 12).
    HighPrice,
    /// Day low premium (field 13).
    LowPrice,
    /// Prior session close premium (field 14).
    ClosePrice,
    /// MIC venue id for the last trade (field 15).
    LastId,
    /// Contract description (field 16).
    Description,
    /// Day open premium (field 17).
    OpenPrice,
    /// Open interest, contracts (field 18; double per Schwab's spec).
    OpenInterest,
    /// Mark price (field 19).
    Mark,
    /// Minimum tick size (field 20).
    Tick,
    /// Notional value of one tick, USD (field 21).
    TickAmount,
    /// Underlying-future contract multiplier (field 22).
    FutureMultiplier,
    /// Underlying-future settlement price (field 23).
    FutureSettlementPrice,
    /// Underlying-future symbol (field 24).
    UnderlyingSymbol,
    /// Strike price (field 25).
    StrikePrice,
    /// Expiration date, epoch milliseconds (field 26).
    FutureExpirationDate,
    /// Expiration style description (field 27).
    ExpirationStyle,
    /// Put/call discriminator (`"P"`/`"C"`) (field 28).
    ContractType,
    /// Security status string (field 29).
    SecurityStatus,
    /// Schwab exchange code (field 30).
    Exchange,
    /// Exchange display name (field 31).
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
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(default)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the futures-option symbol).
    pub key: String,
    /// `true` if the quote is delayed.
    pub delayed: bool,
    /// Asset class string (`"FUTURE_OPTION"`).
    #[serde(rename = "assetMainType")]
    pub asset_main_type: Option<String>,
    /// Asset sub-type string.
    #[serde(rename = "assetSubType")]
    pub asset_sub_type: Option<String>,
    /// CUSIP, when Schwab supplies one.
    pub cusip: Option<String>,

    /// Field 0: wire symbol.
    pub symbol: Option<String>,
    /// Field 1: best bid premium.
    #[serde(with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Field 2: best ask premium.
    #[serde(with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Field 3: last trade premium.
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
    /// Field 12: day high premium.
    #[serde(with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Field 13: day low premium.
    #[serde(with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Field 14: prior session close premium.
    #[serde(with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Field 15: MIC venue id for the last trade.
    pub last_id: Option<String>,
    /// Field 16: contract description.
    pub description: Option<String>,
    /// Field 17: day open premium.
    #[serde(with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Field 18: open interest (Schwab types this as `double` for
    /// futures-options).
    #[serde(with = "decimal_opt")]
    pub open_interest: Option<Decimal>,
    /// Field 19: mark price.
    #[serde(with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Field 20: minimum tick size.
    #[serde(with = "decimal_opt")]
    pub tick: Option<Decimal>,
    /// Field 21: notional value of one tick, USD.
    #[serde(with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    /// Field 22: underlying-future contract multiplier.
    #[serde(with = "decimal_opt")]
    pub future_multiplier: Option<Decimal>,
    /// Field 23: underlying-future settlement price.
    #[serde(with = "decimal_opt")]
    pub future_settlement_price: Option<Decimal>,
    /// Field 24: underlying-future symbol.
    pub underlying_symbol: Option<String>,
    /// Field 25: strike price.
    #[serde(with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    /// Field 26: expiration date, epoch milliseconds.
    pub future_expiration_date: Option<i64>,
    /// Field 27: expiration style description.
    pub expiration_style: Option<String>,
    /// Field 28: put/call discriminator (`"P"`/`"C"`).
    pub contract_type: Option<String>,
    /// Field 29: security status string.
    pub security_status: Option<String>,
    /// Field 30: Schwab exchange code.
    pub exchange: Option<String>,
    /// Field 31: exchange display name.
    pub exchange_name: Option<String>,
}

impl Content {
    /// Decode a remapped JSON object (numeric keys already resolved to
    /// snake_case names by the streamer frame parser) into a typed batch.
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
    use crate::streamer::StreamerRequest;
    use crate::streamer::StreamerResponse;
    use crate::streamer::response::{DataContent, parse};
    use crate::streamer::subscription::{Command, Subscription, subscribe_parameters};
    use rust_decimal_macros::dec;

    #[test]
    fn parses_level_one_futures_options_data_into_typed_content() {
        let frame = r#"{
            "data": [{
                "service": "LEVELONE_FUTURES_OPTIONS",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "./OZCZ23C565",
                    "delayed": false,
                    "1": 12.25, "2": 12.50, "3": 12.375,
                    "4": 5, "5": 7, "8": 234,
                    "18": 1500.5,
                    "19": 12.375, "20": 0.25, "21": 12.50,
                    "22": 50.0,
                    "24": "/ZCZ23", "25": 565.0,
                    "28": "C"
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::LevelOneFuturesOptions);
        let DataContent::LevelOneFuturesOptions(items) = &payload.content else {
            panic!("expected LevelOneFuturesOptions");
        };
        let item = &items[0];
        assert_eq!(item.key, "./OZCZ23C565");
        assert_eq!(item.bid_price, Some(dec!(12.25)));
        assert_eq!(item.ask_price, Some(dec!(12.50)));
        assert_eq!(item.total_volume, Some(234));
        assert_eq!(item.open_interest, Some(dec!(1500.5))); // double per spec
        assert_eq!(item.mark, Some(dec!(12.375)));
        assert_eq!(item.future_multiplier, Some(dec!(50.0)));
        assert_eq!(item.underlying_symbol.as_deref(), Some("/ZCZ23"));
        assert_eq!(item.strike_price, Some(dec!(565.0)));
        assert_eq!(item.contract_type.as_deref(), Some("C"));
    }

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
