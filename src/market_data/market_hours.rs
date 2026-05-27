//! `/markets` and `/markets/{market_id}` - market hours.
//!
//! Returns Schwab's session windows for one or more markets on a given
//! date. The response is a nested map: outer key is the requested market
//! id (e.g. `"equity"`), inner key is the product within that market
//! (e.g. `"EQ"`, `"EQNX"`), and the value is the [`Hours`] block.
//!
//! Reached through
//! [`MarketData::market_hours`](super::MarketData::market_hours).

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;

/// Accessor for `/markets*`. Construct via
/// [`MarketData::market_hours`](super::MarketData::market_hours).
#[derive(Debug)]
pub struct MarketHours<'a> {
    client: &'a SchwabClient,
}

impl<'a> MarketHours<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Begin a `GET /markets?markets=...` batch request across one or
    /// more markets. The response is keyed by the market id Schwab
    /// returns (lowercase) and may contain multiple product sub-entries
    /// per market.
    pub fn list<I>(&self, markets: I) -> ListMarketHoursBuilder<'a>
    where
        I: IntoIterator<Item = Market>,
    {
        let csv = markets
            .into_iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(",");
        ListMarketHoursBuilder {
            client: self.client,
            markets: csv,
            date: None,
        }
    }

    /// Begin a `GET /markets/{market_id}` single-market request.
    pub fn get(&self, market: Market) -> GetMarketHoursBuilder<'a> {
        GetMarketHoursBuilder {
            client: self.client,
            market,
            date: None,
        }
    }
}

/// In-flight request for `GET /markets`.
#[derive(Debug)]
#[must_use = "call .send() to execute the request"]
pub struct ListMarketHoursBuilder<'a> {
    client: &'a SchwabClient,
    markets: String,
    date: Option<NaiveDate>,
}

impl<'a> ListMarketHoursBuilder<'a> {
    /// Restrict the response to a specific date (Schwab format
    /// `YYYY-MM-DD`). Defaults to today on Schwab's side; the valid
    /// range is today through one year out.
    pub fn date(mut self, date: NaiveDate) -> Self {
        self.date = Some(date);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<MarketHoursResponse> {
        let mut request = self
            .client
            .market_data_http()
            .get("/markets")
            .query(&[("markets", self.markets.as_str())]);
        if let Some(d) = self.date {
            let s = d.format("%Y-%m-%d").to_string();
            request = request.query(&[("date", s.as_str())]);
        }
        request.send_json().await
    }
}

/// In-flight request for `GET /markets/{market_id}`.
#[derive(Debug)]
#[must_use = "call .send() to execute the request"]
pub struct GetMarketHoursBuilder<'a> {
    client: &'a SchwabClient,
    market: Market,
    date: Option<NaiveDate>,
}

impl<'a> GetMarketHoursBuilder<'a> {
    /// Restrict the response to a specific date. Defaults to today.
    pub fn date(mut self, date: NaiveDate) -> Self {
        self.date = Some(date);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<MarketHoursResponse> {
        let path = format!("/markets/{}", self.market);
        let mut request = self.client.market_data_http().get(&path);
        if let Some(d) = self.date {
            let s = d.format("%Y-%m-%d").to_string();
            request = request.query(&[("date", s.as_str())]);
        }
        request.send_json().await
    }
}

// --- Response shape ---

/// `/markets*` response body. Two levels of map:
/// outer key is the market id Schwab returned (lowercase, matching
/// [`Market`]); inner key is the per-market product (e.g. `"EQ"`,
/// `"BOND"`); value is the [`Hours`] block.
pub type MarketHoursResponse = HashMap<String, HashMap<String, Hours>>;

/// Market-hours detail for one product within one market.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Hours {
    /// `yyyy-MM-dd` date the hours apply to.
    #[serde(default)]
    pub date: Option<String>,
    /// Broader market-type classification.
    #[serde(rename = "marketType", default)]
    pub market_type: Option<MarketType>,
    /// Exchange the product trades on, when applicable.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Product category (Schwab-specific subdivision).
    #[serde(default)]
    pub category: Option<String>,
    /// Product code Schwab uses for this entry (e.g. `"EQ"`, `"BOND"`).
    #[serde(default)]
    pub product: Option<String>,
    /// Human-readable product name.
    #[serde(rename = "productName", default)]
    pub product_name: Option<String>,
    /// `true` if any session is scheduled to open on the requested date.
    #[serde(rename = "isOpen", default)]
    pub is_open: Option<bool>,
    /// Session windows keyed by session name (e.g. `"preMarket"`,
    /// `"regularMarket"`, `"postMarket"`, `"outcryMarket"`). Each entry
    /// is a list of contiguous [`Interval`]s in that session.
    #[serde(rename = "sessionHours", default)]
    pub session_hours: HashMap<String, Vec<Interval>>,
}

/// One contiguous session window. `start` and `end` are ISO-8601
/// timestamp strings carrying the exchange-local timezone (e.g.
/// `"2024-03-15T09:30:00-04:00"`); kept as `String` for now since the
/// timezone is informational and chrono parsing is a one-liner at the
/// consumer.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Interval {
    /// Session start timestamp.
    #[serde(default)]
    pub start: Option<String>,
    /// Session end timestamp.
    #[serde(default)]
    pub end: Option<String>,
}

// --- Enums ---

string_enum! {
    /// Market-id query/path value. Lowercase per Schwab's spec.
    Market {
        /// Equity market.
        Equity = "equity",
        /// Listed-options market.
        Option_ = "option",
        /// Fixed-income market.
        Bond = "bond",
        /// Futures market.
        Future = "future",
        /// Foreign-exchange market.
        Forex = "forex",
    }
}

string_enum! {
    /// Broader market-type discriminator on the [`Hours`] response.
    MarketType {
        /// Bond market.
        Bond = "BOND",
        /// Equity market.
        Equity = "EQUITY",
        /// Exchange-traded fund.
        Etf = "ETF",
        /// Extended-hours classification.
        Extended = "EXTENDED",
        /// Forex market.
        Forex = "FOREX",
        /// Futures market.
        Future = "FUTURE",
        /// Futures-option market.
        FutureOption = "FUTURE_OPTION",
        /// Fundamental-data feed.
        Fundamental = "FUNDAMENTAL",
        /// Index.
        Index = "INDEX",
        /// Technical indicator.
        Indicator = "INDICATOR",
        /// Mutual-fund market.
        MutualFund = "MUTUAL_FUND",
        /// Listed-options market.
        Option_ = "OPTION",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equity_market_hours_response_parses() {
        // Modeled on Schwab's documented shape: outer map keyed by
        // market (e.g. "equity"), inner map keyed by product code
        // ("EQ", "NYSE", etc.).
        let json = r#"{
            "equity": {
                "EQ": {
                    "date": "2024-03-15",
                    "marketType": "EQUITY",
                    "exchange": "NULL",
                    "category": "NULL",
                    "product": "EQ",
                    "productName": "equity",
                    "isOpen": true,
                    "sessionHours": {
                        "preMarket": [
                            { "start": "2024-03-15T07:00:00-04:00", "end": "2024-03-15T09:30:00-04:00" }
                        ],
                        "regularMarket": [
                            { "start": "2024-03-15T09:30:00-04:00", "end": "2024-03-15T16:00:00-04:00" }
                        ],
                        "postMarket": [
                            { "start": "2024-03-15T16:00:00-04:00", "end": "2024-03-15T20:00:00-04:00" }
                        ]
                    }
                }
            }
        }"#;
        let resp: MarketHoursResponse = serde_json::from_str(json).unwrap();
        let equity = resp.get("equity").unwrap();
        let eq = equity.get("EQ").unwrap();
        assert_eq!(eq.date.as_deref(), Some("2024-03-15"));
        assert_eq!(eq.market_type, Some(MarketType::Equity));
        assert_eq!(eq.is_open, Some(true));
        assert_eq!(eq.product.as_deref(), Some("EQ"));

        let regular = eq.session_hours.get("regularMarket").unwrap();
        assert_eq!(regular.len(), 1);
        assert_eq!(
            regular[0].start.as_deref(),
            Some("2024-03-15T09:30:00-04:00")
        );
        assert_eq!(regular[0].end.as_deref(), Some("2024-03-15T16:00:00-04:00"));

        let pre = eq.session_hours.get("preMarket").unwrap();
        assert_eq!(pre[0].end.as_deref(), Some("2024-03-15T09:30:00-04:00"));
    }

    #[test]
    fn closed_market_response_parses() {
        // Schwab returns isOpen=false plus an empty sessionHours block
        // when the market is closed on the requested date.
        let json = r#"{
            "equity": {
                "EQ": {
                    "date": "2024-12-25",
                    "marketType": "EQUITY",
                    "product": "EQ",
                    "isOpen": false,
                    "sessionHours": {}
                }
            }
        }"#;
        let resp: MarketHoursResponse = serde_json::from_str(json).unwrap();
        let eq = resp.get("equity").unwrap().get("EQ").unwrap();
        assert_eq!(eq.is_open, Some(false));
        assert!(eq.session_hours.is_empty());
    }

    #[test]
    fn multi_market_response_parses() {
        let json = r#"{
            "equity": { "EQ": { "product": "EQ", "isOpen": true, "sessionHours": {} } },
            "option": { "EQO": { "product": "EQO", "isOpen": true, "sessionHours": {} } }
        }"#;
        let resp: MarketHoursResponse = serde_json::from_str(json).unwrap();
        assert!(resp.contains_key("equity"));
        assert!(resp.contains_key("option"));
    }

    #[test]
    fn market_round_trips_known_variants() {
        for raw in ["equity", "option", "bond", "future", "forex"] {
            let json = format!(r#""{raw}""#);
            let parsed: Market = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn unknown_market_type_preserves_raw_string() {
        let parsed: MarketType = serde_json::from_str(r#""NEW_CLASS""#).unwrap();
        assert!(matches!(parsed, MarketType::Unknown(ref s) if s == "NEW_CLASS"));
    }

    #[test]
    fn naive_date_formats_to_schwab_wire_form() {
        let d = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2024-03-15");
    }
}
