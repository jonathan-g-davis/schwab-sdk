//! `OPTIONS_BOOK` streamer service.
//!
//! Level-2 order book for listed options. See `book::Content`,
//! `book::PriceLevel`, and `book::MarketMaker` for the shared payload shape.
//!
//! Note: distinct from `crate::streamer::level_one::options`, which is the
//! Level-1 quote stream for the same instruments.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::book;
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::OptionsBook;
}

/// Field enum for the OPTIONS_BOOK service. Identical layout to NYSE_BOOK and
/// NASDAQ_BOOK but a distinct type so the `SubscriptionField` impl can bind
/// the correct `Service` variant.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    serde_repr::Serialize_repr,
    Display,
    EnumString,
    FromRepr,
)]
#[repr(u8)]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum Field {
    /// Wire symbol (field 0).
    Symbol,
    /// Snapshot timestamp, epoch milliseconds (field 1).
    MarketSnapshotTime,
    /// Bid-side price levels (field 2).
    BidSideLevels,
    /// Ask-side price levels (field 3).
    AskSideLevels,
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

pub(crate) fn decode_batch(remapped: serde_json::Value) -> Result<Vec<book::Content>> {
    book::decode_batch(remapped, "OPTIONS_BOOK")
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
    fn parses_options_book_data_into_typed_content() {
        // Same shape as NYSE/NASDAQ book; instrument key is a Schwab option
        // symbol. One bid level, one ask level, single MM each.
        let frame = r#"{
            "data": [{
                "service": "OPTIONS_BOOK",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL  240315C00200000",
                    "delayed": false,
                    "1": 1714949592300,
                    "2": [{
                        "0": 5.10,
                        "1": 12,
                        "2": 1,
                        "3": [{"0": "MMX", "1": 12, "2": 1714949592000}]
                    }],
                    "3": [{
                        "0": 5.20,
                        "1": 8,
                        "2": 1,
                        "3": [{"0": "MMY", "1": 8, "2": 1714949592200}]
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::OptionsBook);
        let DataContent::OptionsBook(items) = &payload.content else {
            panic!("expected OptionsBook, got {:?}", payload.content);
        };
        let opt = &items[0];
        assert_eq!(opt.key, "AAPL  240315C00200000");
        assert_eq!(opt.market_snapshot_time, 1714949592300);

        assert_eq!(opt.bid_side_levels.len(), 1);
        assert_eq!(opt.bid_side_levels[0].price, dec!(5.10));
        assert_eq!(opt.bid_side_levels[0].aggregate_size, 12);
        assert_eq!(
            opt.bid_side_levels[0].market_makers[0].market_maker_id,
            "MMX"
        );

        assert_eq!(opt.ask_side_levels.len(), 1);
        assert_eq!(opt.ask_side_levels[0].price, dec!(5.20));
        assert_eq!(
            opt.ask_side_levels[0].market_makers[0].market_maker_id,
            "MMY"
        );
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["AAPL  240315C00200000".to_string()],
            vec![
                Field::Symbol,
                Field::MarketSnapshotTime,
                Field::BidSideLevels,
                Field::AskSideLevels,
            ],
        );
        assert_eq!(value["keys"], "AAPL  240315C00200000");
        assert_eq!(value["fields"], "0,1,2,3");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["AAPL  240315C00200000".to_string()],
            fields: vec![Field::BidSideLevels, Field::AskSideLevels],
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
        assert_eq!(
            Field::MarketSnapshotTime.to_string(),
            "market_snapshot_time"
        );
        assert_eq!(Field::BidSideLevels.to_string(), "bid_side_levels");
        assert_eq!(Field::AskSideLevels.to_string(), "ask_side_levels");
    }
}
