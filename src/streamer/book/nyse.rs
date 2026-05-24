//! `NYSE_BOOK` streamer service.
//!
//! Level-2 order book for NYSE-listed equities. See `book::Content`,
//! `book::PriceLevel`, and `book::MarketMaker` for the shared payload shape.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::book;
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::NyseBook;
}

/// Field enum for the NYSE_BOOK service. Identical layout to NASDAQ_BOOK and
/// OPTIONS_BOOK but a distinct type so the `SubscriptionField` impl can bind
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
    Symbol,
    MarketSnapshotTime,
    BidSideLevels,
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
    book::decode_batch(remapped, "NYSE_BOOK")
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
    fn parses_nyse_book_data_into_typed_content() {
        // One bid level at 150.00 with two market makers and one ask level
        // at 150.05 with a single market maker.
        let frame = r#"{
            "data": [{
                "service": "NYSE_BOOK",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "AAPL",
                    "delayed": false,
                    "1": 1714949592300,
                    "2": [{
                        "0": 150.00,
                        "1": 1000,
                        "2": 2,
                        "3": [
                            {"0": "MM1", "1": 600, "2": 1714949592000},
                            {"0": "MM2", "1": 400, "2": 1714949592100}
                        ]
                    }],
                    "3": [{
                        "0": 150.05,
                        "1": 500,
                        "2": 1,
                        "3": [{"0": "MM3", "1": 500, "2": 1714949592200}]
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::NyseBook);
        let DataContent::NyseBook(items) = &payload.content else {
            panic!("expected NyseBook, got {:?}", payload.content);
        };
        let aapl = &items[0];
        assert_eq!(aapl.key, "AAPL");
        assert_eq!(aapl.market_snapshot_time, 1714949592300);

        assert_eq!(aapl.bid_side_levels.len(), 1);
        let bid = &aapl.bid_side_levels[0];
        assert_eq!(bid.price, dec!(150.00));
        assert_eq!(bid.aggregate_size, 1000);
        assert_eq!(bid.market_maker_count, 2);
        assert_eq!(bid.market_makers.len(), 2);
        assert_eq!(bid.market_makers[0].market_maker_id, "MM1");
        assert_eq!(bid.market_makers[0].size, 600);
        assert_eq!(bid.market_makers[0].quote_time, 1714949592000);

        assert_eq!(aapl.ask_side_levels.len(), 1);
        let ask = &aapl.ask_side_levels[0];
        assert_eq!(ask.price, dec!(150.05));
        assert_eq!(ask.aggregate_size, 500);
        assert_eq!(ask.market_maker_count, 1);
        assert_eq!(ask.market_makers[0].market_maker_id, "MM3");
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["AAPL".to_string()],
            vec![
                Field::Symbol,
                Field::MarketSnapshotTime,
                Field::BidSideLevels,
                Field::AskSideLevels,
            ],
        );
        assert_eq!(value["keys"], "AAPL");
        assert_eq!(value["fields"], "0,1,2,3");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["AAPL".to_string(), "IBM".to_string()],
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
