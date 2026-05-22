//! `/movers/{symbol_id}` - top movers for an index.
//!
//! Returns the top-moving securities within an index, optionally sorted
//! and windowed.
//!
//! Reached through
//! [`MarketData::movers`](super::MarketData::movers).

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::{Deserialize, Serialize};

use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;

/// Accessor for `/movers/{symbol_id}`. Construct via
/// [`MarketData::movers`](super::MarketData::movers).
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

    pub async fn send(self) -> Result<MoversResponse> {
        let md = self.client.market_data_http();
        let path = format!("/movers/{}", self.index);
        let mut request = md.get(&path);
        if let Some(sort) = &self.sort {
            let s = sort.to_string();
            request = request.query(&[("sort", s.as_str())]);
        }
        if let Some(freq) = self.frequency {
            let s = freq.to_string();
            request = request.query(&[("frequency", s.as_str())]);
        }
        md.execute_json(request).await
    }
}

// --- Response shape ---

/// `/movers/{symbol_id}` response body.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MoversResponse {
    #[serde(default)]
    pub screeners: Vec<Screener>,
}

/// One moved security within an index.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Screener {
    /// Percent (default) or value changed. Sign is informational; pair
    /// with [`Self::direction`] for the explicit up/down.
    #[serde(default, with = "decimal_opt")]
    pub change: Option<Decimal>,
    /// Name of the security.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub direction: Option<MoverDirection>,
    /// Last quoted price.
    #[serde(default, with = "decimal_opt")]
    pub last: Option<Decimal>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
}

// --- Enums ---

string_enum! {
    /// Index symbol path value for the movers endpoint.
    MoverIndex {
        Dji = "$DJI",
        Compx = "$COMPX",
        Spx = "$SPX",
        Nyse = "NYSE",
        Nasdaq = "NASDAQ",
        Otcbb = "OTCBB",
        IndexAll = "INDEX_ALL",
        EquityAll = "EQUITY_ALL",
        OptionAll = "OPTION_ALL",
        OptionPut = "OPTION_PUT",
        OptionCall = "OPTION_CALL",
    }
}

string_enum! {
    /// `sort` query value for the movers endpoint.
    MoverSort {
        Volume = "VOLUME",
        Trades = "TRADES",
        PercentChangeUp = "PERCENT_CHANGE_UP",
        PercentChangeDown = "PERCENT_CHANGE_DOWN",
    }
}

string_enum! {
    /// Direction of a mover's price change.
    MoverDirection {
        Up = "up",
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
