//! `/movers/{symbol_id}` - top movers for an index.
//!
//! Returns the top-moving securities within an index, optionally sorted
//! and windowed.
//!
//! The `index` path segment is a [`MoverIndex`]; both broad indices (`$DJI`,
//! `$COMPX`, `$SPX`) and Schwab venue aggregates (`NYSE`, `NASDAQ`,
//! `INDEX_ALL`, `EQUITY_ALL`, `OPTION_ALL`, `OPTION_PUT`, `OPTION_CALL`) are
//! valid. The optional `sort` ([`MoverSort`]) selects the ranking key, and
//! the optional `frequency` is an aggregation window in minutes. Schwab
//! documents valid `frequency` values as `{0, 1, 5, 10, 30, 60}` with a
//! default of `0`. Out-of-range values surface as a 400.
//!
//! Reached through
//! [`MarketData::movers`](super::MarketData::movers).
//!
//! # Example
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient};
//! use schwab_sdk::market_data::{MoverIndex, MoverSort};
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! let movers = client
//!     .market_data()
//!     .movers()
//!     .get(MoverIndex::Spx)
//!     .sort(MoverSort::PercentChangeUp)
//!     .send()
//!     .await?;
//!
//! for screener in movers.screeners.iter().take(10) {
//!     println!(
//!         "{:?} {:?} {:?}",
//!         screener.symbol, screener.change, screener.direction,
//!     );
//! }
//! # Ok(())
//! # }
//! ```

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;

/// Accessor for `/movers/{symbol_id}`. Construct via
/// [`MarketData::movers`](super::MarketData::movers).
#[derive(Debug)]
pub struct Movers<'a> {
    client: &'a SchwabClient,
}

impl<'a> Movers<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Begin a `GET /movers/{symbol_id}` request for an index.
    pub fn get(&self, index: MoverIndex) -> GetMoversBuilder<'a> {
        GetMoversBuilder {
            client: self.client,
            index,
            sort: None,
            frequency: None,
        }
    }
}

/// In-flight request for `GET /movers/{symbol_id}`. Built via
/// [`Movers::get`].
#[derive(Debug)]
#[must_use = "call .send() to execute the request"]
pub struct GetMoversBuilder<'a> {
    client: &'a SchwabClient,
    index: MoverIndex,
    sort: Option<MoverSort>,
    frequency: Option<i32>,
}

impl<'a> GetMoversBuilder<'a> {
    /// Sort the movers by a particular attribute.
    pub fn sort(mut self, sort: MoverSort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Aggregation window in minutes. Schwab documents valid values as
    /// `{0, 1, 5, 10, 30, 60}` with a default of `0`; out-of-range
    /// values surface as a 400.
    pub fn frequency(mut self, minutes: i32) -> Self {
        self.frequency = Some(minutes);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<MoversResponse> {
        let path = format!("/movers/{}", self.index);
        let mut request = self.client.market_data_http().get(&path);
        if let Some(sort) = &self.sort {
            let s = sort.to_string();
            request = request.query(&[("sort", s.as_str())]);
        }
        if let Some(freq) = self.frequency {
            let s = freq.to_string();
            request = request.query(&[("frequency", s.as_str())]);
        }
        request.send_json().await
    }
}

// --- Response shape ---

/// `/movers/{symbol_id}` response body.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MoversResponse {
    /// One entry per top-moving security in the index.
    #[serde(default)]
    pub screeners: Vec<Screener>,
}

/// One moved security within an index.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Screener {
    /// Percent (default) or value changed. Sign is informational; pair
    /// with [`Self::direction`] for the explicit up/down.
    #[serde(default, with = "decimal_opt")]
    pub change: Option<Decimal>,
    /// Name of the security.
    #[serde(default)]
    pub description: Option<String>,
    /// Up/down direction of the move.
    #[serde(default)]
    pub direction: Option<MoverDirection>,
    /// Last quoted price.
    #[serde(default, with = "decimal_opt")]
    pub last: Option<Decimal>,
    /// Wire symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Cumulative session volume (shares/contracts).
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
}

// --- Enums ---

string_enum! {
    /// Index symbol path value for the movers endpoint.
    MoverIndex {
        /// Dow Jones Industrial Average.
        Dji = "$DJI",
        /// Nasdaq Composite.
        Compx = "$COMPX",
        /// S&P 500.
        Spx = "$SPX",
        /// NYSE-listed.
        Nyse = "NYSE",
        /// Nasdaq-listed.
        Nasdaq = "NASDAQ",
        /// OTC Bulletin Board.
        Otcbb = "OTCBB",
        /// All indices.
        IndexAll = "INDEX_ALL",
        /// All equities.
        EquityAll = "EQUITY_ALL",
        /// All options.
        OptionAll = "OPTION_ALL",
        /// All puts.
        OptionPut = "OPTION_PUT",
        /// All calls.
        OptionCall = "OPTION_CALL",
    }
}

string_enum! {
    /// `sort` query value for the movers endpoint.
    MoverSort {
        /// Sort by cumulative volume.
        Volume = "VOLUME",
        /// Sort by trade count.
        Trades = "TRADES",
        /// Sort by upward percent change.
        PercentChangeUp = "PERCENT_CHANGE_UP",
        /// Sort by downward percent change.
        PercentChangeDown = "PERCENT_CHANGE_DOWN",
    }
}

string_enum! {
    /// Direction of a mover's price change.
    MoverDirection {
        /// Price moved up.
        Up = "up",
        /// Price moved down.
        Down = "down",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn movers_response_parses() {
        let json = r#"{
            "screeners": [
                {
                    "symbol": "AAPL",
                    "description": "Apple Inc.",
                    "direction": "up",
                    "change": 0.0314,
                    "last": 145.32,
                    "totalVolume": 50000000
                },
                {
                    "symbol": "TSLA",
                    "description": "Tesla Inc.",
                    "direction": "down",
                    "change": -0.0212,
                    "last": 240.15,
                    "totalVolume": 80000000
                }
            ]
        }"#;
        let resp: MoversResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.screeners.len(), 2);

        let aapl = &resp.screeners[0];
        assert_eq!(aapl.symbol.as_deref(), Some("AAPL"));
        assert_eq!(aapl.direction, Some(MoverDirection::Up));
        assert_eq!(aapl.change, Some(dec!(0.0314)));
        assert_eq!(aapl.last, Some(dec!(145.32)));
        assert_eq!(aapl.total_volume, Some(50000000));

        let tsla = &resp.screeners[1];
        assert_eq!(tsla.direction, Some(MoverDirection::Down));
        assert_eq!(tsla.change, Some(dec!(-0.0212)));
    }

    #[test]
    fn empty_movers_response_parses() {
        let resp: MoversResponse = serde_json::from_str(r#"{"screeners": []}"#).unwrap();
        assert!(resp.screeners.is_empty());
    }

    #[test]
    fn movers_response_with_missing_screeners_defaults_empty() {
        let resp: MoversResponse = serde_json::from_str("{}").unwrap();
        assert!(resp.screeners.is_empty());
    }

    #[test]
    fn mover_index_round_trips_dollar_prefixed_variants() {
        for raw in ["$DJI", "$COMPX", "$SPX", "NYSE", "OPTION_CALL"] {
            let json = format!(r#""{raw}""#);
            let parsed: MoverIndex = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn mover_index_display_keeps_dollar_prefix() {
        // The builder interpolates this into the URL path; the `$`
        // prefix must survive.
        assert_eq!(MoverIndex::Dji.to_string(), "$DJI");
        assert_eq!(MoverIndex::Spx.to_string(), "$SPX");
    }

    #[test]
    fn mover_sort_round_trips_known_variants() {
        for raw in [
            "VOLUME",
            "TRADES",
            "PERCENT_CHANGE_UP",
            "PERCENT_CHANGE_DOWN",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: MoverSort = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn unknown_mover_direction_preserves_raw_string() {
        let parsed: MoverDirection = serde_json::from_str(r#""sideways""#).unwrap();
        assert!(matches!(parsed, MoverDirection::Unknown(ref s) if s == "sideways"));
    }
}
