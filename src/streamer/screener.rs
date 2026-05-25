//! Screener (advancers / decliners / actives) streamer services.
//!
//! Schwab publishes two screener services with the same payload shape:
//! `SCREENER_EQUITY` and `SCREENER_OPTION`. Each subscribed key is a
//! composite identifier of the form `PREFIX_SORTFIELD_FREQUENCY`, e.g.
//! `NYSE_VOLUME_5` or `OPTION_PUT_PERCENT_CHANGE_UP_1`.
//!
//! Delivery type is "Whole": each tick carries the full ranking snapshot.
//! Top-level fields arrive with numeric-string keys (remapped by the
//! streamer frame parser). Items inside the `items` array use named
//! camelCase fields and decode via standard serde rename rules.

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use crate::error::{Error, Result};

pub mod equity;
pub mod option;

/// One screener result row.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Content {
    /// Subscription key (the composite screener identifier).
    pub key: String,
    /// `true` if the snapshot is delayed.
    pub delayed: bool,
    /// Field 0. The symbol used to look up actives, gainers, or losers; in
    /// practice the subscribed composite key.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Field 1. Market snapshot timestamp, milliseconds since the Unix epoch.
    #[serde(default)]
    pub timestamp: Option<u64>,
    /// Field 2. The field the rankings were sorted on.
    #[serde(default)]
    pub sort_field: Option<String>,
    /// Field 3. Aggregation window in minutes (0 = all day, otherwise 1, 5,
    /// 10, 30, or 60).
    #[serde(default)]
    pub frequency: Option<i32>,
    /// Field 4. The ranked instruments.
    #[serde(default)]
    pub items: Vec<Item>,
}

/// A single ranked instrument inside `Content::items`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[non_exhaustive]
pub struct Item {
    /// Instrument description.
    pub description: Option<String>,
    /// Last trade price, USD.
    #[serde(with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Market share percentage of the instrument.
    #[serde(with = "decimal_opt")]
    pub market_share: Option<Decimal>,
    /// Net change since prior close, USD.
    #[serde(with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Net change since prior close as a fraction.
    #[serde(with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Wire symbol.
    pub symbol: Option<String>,
    /// Cumulative session volume.
    pub total_volume: Option<u64>,
    /// Number of trades observed during the requested frequency window.
    pub trades: Option<i64>,
    /// Volume observed during the requested frequency window.
    pub volume: Option<u64>,
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
