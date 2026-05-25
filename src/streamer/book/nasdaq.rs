//! `NASDAQ_BOOK` streamer service.
//!
//! Level-2 order book for NASDAQ-listed equities. See `book::Content`,
//! `book::PriceLevel`, and `book::MarketMaker` for the shared payload shape.

use serde::Deserialize;
use strum::{Display, EnumString, FromRepr};

use crate::error::Result;
use crate::streamer::book;
use crate::streamer::{Service, subscription::SubscriptionField};

impl SubscriptionField for Field {
    const SERVICE: Service = Service::NasdaqBook;
}

/// Field enum for the NASDAQ_BOOK service. Identical layout to NYSE_BOOK and
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
    book::decode_batch(remapped, "NASDAQ_BOOK")
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
    fn parses_nasdaq_book_data_into_typed_content() {
        // Two bid levels (one with two MMs) and one ask level. Mirrors the
        // shape NYSE_BOOK uses since both share `book::Content`.
        let frame = r#"{
            "data": [{
                "service": "NASDAQ_BOOK",
                "timestamp": 1714949592301,
                "command": "SUBS",
                "content": [{
                    "key": "MSFT",
                    "delayed": false,
                    "1": 1714949592300,
                    "2": [
                        {
                            "0": 425.10,
                            "1": 800,
                            "2": 2,
                            "3": [
                                {"0": "MMA", "1": 500, "2": 1714949592000},
                                {"0": "MMB", "1": 300, "2": 1714949592100}
                            ]
                        },
                        {
                            "0": 425.05,
                            "1": 1200,
                            "2": 1,
                            "3": [{"0": "MMC", "1": 1200, "2": 1714949591900}]
                        }
                    ],
                    "3": [{
                        "0": 425.15,
                        "1": 600,
                        "2": 1,
                        "3": [{"0": "MMD", "1": 600, "2": 1714949592250}]
                    }]
                }]
            }]
        }"#;
        let StreamerResponse::Data(data) = parse(frame).unwrap() else {
            panic!("expected Data");
        };
        let payload = &data[0];
        assert_eq!(payload.service, Service::NasdaqBook);
        let DataContent::NasdaqBook(items) = &payload.content else {
            panic!("expected NasdaqBook, got {:?}", payload.content);
        };
        let msft = &items[0];
        assert_eq!(msft.key, "MSFT");
        assert_eq!(msft.market_snapshot_time, 1714949592300);

        assert_eq!(msft.bid_side_levels.len(), 2);
        assert_eq!(msft.bid_side_levels[0].price, dec!(425.10));
        assert_eq!(msft.bid_side_levels[0].aggregate_size, 800);
        assert_eq!(msft.bid_side_levels[0].market_makers.len(), 2);
        assert_eq!(msft.bid_side_levels[1].price, dec!(425.05));
        assert_eq!(msft.bid_side_levels[1].market_maker_count, 1);

        assert_eq!(msft.ask_side_levels.len(), 1);
        assert_eq!(msft.ask_side_levels[0].price, dec!(425.15));
        assert_eq!(
            msft.ask_side_levels[0].market_makers[0].market_maker_id,
            "MMD"
        );
    }

    #[test]
    fn fields_serialize_as_numeric_index() {
        let value = subscribe_parameters(
            vec!["MSFT".to_string()],
            vec![
                Field::Symbol,
                Field::MarketSnapshotTime,
                Field::BidSideLevels,
                Field::AskSideLevels,
            ],
        );
        assert_eq!(value["keys"], "MSFT");
        assert_eq!(value["fields"], "0,1,2,3");
    }

    #[test]
    fn from_subscription_never_panics() {
        let sub = Subscription {
            command: Command::Subscribe,
            keys: vec!["MSFT".to_string(), "AAPL".to_string()],
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
