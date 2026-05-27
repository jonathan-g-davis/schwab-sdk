//! Level-2 order book streamer services.
//!
//! Shared payload shape across `NYSE_BOOK`, `NASDAQ_BOOK`, and `OPTIONS_BOOK`.
//! Each service has its own `Field` enum because the three services route to
//! different `Service` variants and a single `From<Subscription<Field>>` impl
//! cannot dispatch by type parameter; the sub-structs below are shared.
//!
//! Delivery type for all BOOK services is "Whole": the entire book snapshot
//! is sent on each tick. Top-level fields are present on every message, so
//! they are non-Optional in `Content`. Nested levels arrive with
//! numeric-string keys (`"0"`, `"1"`, ...); `#[serde(rename)]` resolves them
//! without extending the top-level `transform_keys`.

use rust_decimal::Decimal;
use rust_decimal::serde::float as decimal_float;
use serde::Deserialize;

use crate::error::{Error, Result};

pub mod nasdaq;
pub mod nyse;
pub mod options;

/// Top-level book payload. One per subscribed symbol per tick.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the symbol the book is for).
    pub key: String,
    /// `true` if the book is delayed.
    pub delayed: bool,
    /// Field 0. Schwab echoes the symbol back here on some ticks; usually
    /// duplicates `key`.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Field 1. Milliseconds since the Unix epoch.
    pub market_snapshot_time: u64,
    /// Field 2. Bid-side price levels, deepest first per Schwab's wire order.
    pub bid_side_levels: Vec<PriceLevel>,
    /// Field 3. Ask-side price levels.
    pub ask_side_levels: Vec<PriceLevel>,
}

/// A single price level on one side of the book.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct PriceLevel {
    /// Sub-field 0.
    #[serde(rename = "0", with = "decimal_float")]
    pub price: Decimal,
    /// Sub-field 1.
    #[serde(rename = "1")]
    pub aggregate_size: u64,
    /// Sub-field 2.
    #[serde(rename = "2")]
    pub market_maker_count: u32,
    /// Sub-field 3.
    #[serde(rename = "3")]
    pub market_makers: Vec<MarketMaker>,
}

/// A single market maker's contribution to a price level.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MarketMaker {
    /// Sub-field 0.
    #[serde(rename = "0")]
    pub market_maker_id: String,
    /// Sub-field 1.
    #[serde(rename = "1")]
    pub size: u64,
    /// Sub-field 2. Milliseconds since the Unix epoch.
    #[serde(rename = "2")]
    pub quote_time: u64,
}

pub(crate) fn decode_batch(
    remapped: serde_json::Value,
    service_label: &str,
) -> Result<Vec<Content>> {
    serde_json::from_value(remapped).map_err(|e| Error::Codec {
        context: format!("{service_label} content"),
        reason: e.to_string(),
    })
}
