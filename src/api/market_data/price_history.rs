//! `/pricehistory` - OHLCV candles for a single symbol over a date
//! range, at a configurable aggregation.
//!
//! Reached through
//! [`MarketData::price_history`](super::MarketData::price_history).
//!
//! ## Schwab's `periodType`/`period`/`frequencyType`/`frequency` matrix
//!
//! The valid combinations are constrained; Schwab will reject mismatches
//! at the server. The OpenAPI spec documents them as:
//!
//! - `periodType=day`   → `period`  in {1, 2, 3, 4, 5, 10} (default 10),
//!   `frequencyType=minute` (default), `frequency` in {1, 5, 10, 15, 30}
//! - `periodType=month` → `period`  in {1, 2, 3, 6} (default 1),
//!   `frequencyType` in {daily, weekly} (default weekly)
//! - `periodType=year`  → `period`  in {1, 2, 3, 5, 10, 15, 20} (default 1),
//!   `frequencyType` in {daily, weekly, monthly} (default monthly)
//! - `periodType=ytd`   → `period=1` (default 1),
//!   `frequencyType` in {daily, weekly} (default weekly)
//!
//! This builder accepts any `i32`/enum combination; out-of-range values
//! surface as 400 from Schwab rather than being rejected at compile time.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::{Deserialize, Serialize};

use crate::api::macros::string_enum;
use crate::error::Result;
use crate::rest::SchwabClient;

/// Accessor for `/pricehistory`. Construct via
/// [`MarketData::price_history`](super::MarketData::price_history).
pub struct PriceHistory<'a> {
    client: &'a SchwabClient,
}

impl<'a> PriceHistory<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Begin a `GET /pricehistory` request for `symbol`. Schwab applies
    /// default `periodType`, `period`, `frequencyType`, `frequency`, and
    /// date-window values if not specified; see this module's docs for
    /// the documented matrix.
    pub fn get(&self, symbol: impl Into<String>) -> GetPriceHistoryBuilder<'a> {
        GetPriceHistoryBuilder {
            client: self.client,
            symbol: symbol.into(),
            period_type: None,
            period: None,
            frequency_type: None,
            frequency: None,
            start_date: None,
            end_date: None,
            need_extended_hours_data: None,
            need_previous_close: None,
        }
    }
}

/// In-flight request for `GET /pricehistory`. Built via
/// [`PriceHistory::get`].
#[must_use = "call .send() to execute the request"]
pub struct GetPriceHistoryBuilder<'a> {
    client: &'a SchwabClient,
    symbol: String,
    period_type: Option<PeriodType>,
    period: Option<i32>,
    frequency_type: Option<FrequencyType>,
    frequency: Option<i32>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    need_extended_hours_data: Option<bool>,
    need_previous_close: Option<bool>,
}

impl<'a> GetPriceHistoryBuilder<'a> {
    pub fn period_type(mut self, value: PeriodType) -> Self {
        self.period_type = Some(value);
        self
    }

    pub fn period(mut self, value: i32) -> Self {
        self.period = Some(value);
        self
    }

    pub fn frequency_type(mut self, value: FrequencyType) -> Self {
        self.frequency_type = Some(value);
        self
    }

    pub fn frequency(mut self, value: i32) -> Self {
        self.frequency = Some(value);
        self
    }

    /// Bound the lower end of the candle window. Converted to epoch
    /// milliseconds internally - Schwab's wire form for this parameter.
    pub fn start_date(mut self, value: DateTime<Utc>) -> Self {
        self.start_date = Some(value.timestamp_millis());
        self
    }

    /// Bound the upper end of the candle window. Converted to epoch
    /// milliseconds internally.
    pub fn end_date(mut self, value: DateTime<Utc>) -> Self {
        self.end_date = Some(value.timestamp_millis());
        self
    }

    /// Lower-level escape hatch: pass an epoch-milliseconds `start_date`
    /// directly. Useful when feeding values from a wire payload that
    /// already carries milliseconds.
    pub fn start_date_millis(mut self, ms: i64) -> Self {
        self.start_date = Some(ms);
        self
    }

    /// Lower-level escape hatch for `end_date` (epoch milliseconds).
    pub fn end_date_millis(mut self, ms: i64) -> Self {
        self.end_date = Some(ms);
        self
    }

    /// Include pre-/post-market candles. Default Schwab-side is `true`.
    pub fn need_extended_hours_data(mut self, value: bool) -> Self {
        self.need_extended_hours_data = Some(value);
        self
    }

    /// Populate [`CandleList::previous_close`] / `previous_close_date`
    /// in the response. Default Schwab-side is `false`.
    pub fn need_previous_close(mut self, value: bool) -> Self {
        self.need_previous_close = Some(value);
        self
    }

    pub async fn send(self) -> Result<CandleList> {
        let md = self.client.market_data_http();
        let mut request = md
            .get("/pricehistory")
            .query(&[("symbol", self.symbol.as_str())]);
        if let Some(pt) = &self.period_type {
            let s = pt.to_string();
            request = request.query(&[("periodType", s.as_str())]);
        }
        if let Some(p) = self.period {
            let s = p.to_string();
            request = request.query(&[("period", s.as_str())]);
        }
        if let Some(ft) = &self.frequency_type {
            let s = ft.to_string();
            request = request.query(&[("frequencyType", s.as_str())]);
        }
        if let Some(f) = self.frequency {
            let s = f.to_string();
            request = request.query(&[("frequency", s.as_str())]);
        }
        if let Some(sd) = self.start_date {
            let s = sd.to_string();
            request = request.query(&[("startDate", s.as_str())]);
        }
        if let Some(ed) = self.end_date {
            let s = ed.to_string();
            request = request.query(&[("endDate", s.as_str())]);
        }
        if let Some(b) = self.need_extended_hours_data {
            let s = if b { "true" } else { "false" };
            request = request.query(&[("needExtendedHoursData", s)]);
        }
        if let Some(b) = self.need_previous_close {
            let s = if b { "true" } else { "false" };
            request = request.query(&[("needPreviousClose", s)]);
        }
        md.execute_json(request).await
    }
}

// --- Response shape ---

/// `/pricehistory` response body.
#[derive(Debug, Clone, Deserialize)]
pub struct CandleList {
    #[serde(default)]
    pub candles: Vec<Candle>,
    /// `true` when Schwab returned zero candles for the request window.
    #[serde(default)]
    pub empty: bool,
    /// Populated only when the request set `need_previous_close=true`.
    #[serde(default, with = "decimal_opt", rename = "previousClose")]
    pub previous_close: Option<Decimal>,
    /// Epoch milliseconds.
    #[serde(default, rename = "previousCloseDate")]
    pub previous_close_date: Option<i64>,
    /// `yyyy-MM-dd` string companion to [`Self::previous_close_date`].
    #[serde(default, rename = "previousCloseDateISO8601")]
    pub previous_close_date_iso8601: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
}

/// One OHLCV candle.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Candle {
    #[serde(default, with = "decimal_opt")]
    pub open: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub high: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub low: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub close: Option<Decimal>,
    /// Candle-open timestamp in epoch milliseconds.
    #[serde(default)]
    pub datetime: Option<i64>,
    /// `yyyy-MM-dd` string companion to [`Self::datetime`]; Schwab
    /// includes this on daily / weekly / monthly aggregations.
    #[serde(default, rename = "datetimeISO8601")]
    pub datetime_iso8601: Option<String>,
    #[serde(default)]
    pub volume: Option<i64>,
}

// --- Query enums ---

string_enum! {
    /// `periodType` query parameter.
    PeriodType {
        Day = "day",
        Month = "month",
        Year = "year",
        Ytd = "ytd",
    }
}

string_enum! {
    /// `frequencyType` query parameter (the candle aggregation).
    FrequencyType {
        Minute = "minute",
        Daily = "daily",
        Weekly = "weekly",
        Monthly = "monthly",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn candle_list_with_minute_bars_parses() {
        // Shape modeled on Schwab's documented response: a few minute
        // candles plus the previous-close block.
        let json = r#"{
            "symbol": "AAPL",
            "empty": false,
            "previousClose": 145.32,
            "previousCloseDate": 1710374400000,
            "previousCloseDateISO8601": "2024-03-14",
            "candles": [
                {
                    "datetime": 1710423000000,
                    "datetimeISO8601": "2024-03-14",
                    "open": 145.30,
                    "high": 145.50,
                    "low": 145.10,
                    "close": 145.45,
                    "volume": 12345
                },
                {
                    "datetime": 1710423060000,
                    "open": 145.45,
                    "high": 145.55,
                    "low": 145.30,
                    "close": 145.40,
                    "volume": 9876
                }
            ]
        }"#;
        let resp: CandleList = serde_json::from_str(json).unwrap();
        assert_eq!(resp.symbol.as_deref(), Some("AAPL"));
        assert!(!resp.empty);
        assert_eq!(resp.previous_close, Some(dec!(145.32)));
        assert_eq!(resp.previous_close_date, Some(1710374400000));
        assert_eq!(resp.candles.len(), 2);

        let c0 = &resp.candles[0];
        assert_eq!(c0.open, Some(dec!(145.30)));
        assert_eq!(c0.high, Some(dec!(145.50)));
        assert_eq!(c0.low, Some(dec!(145.10)));
        assert_eq!(c0.close, Some(dec!(145.45)));
        assert_eq!(c0.volume, Some(12345));
        assert_eq!(c0.datetime, Some(1710423000000));
        assert_eq!(c0.datetime_iso8601.as_deref(), Some("2024-03-14"));

        let c1 = &resp.candles[1];
        assert_eq!(c1.datetime, Some(1710423060000));
        assert_eq!(c1.datetime_iso8601, None);
    }

    #[test]
    fn empty_candle_list_parses() {
        let json = r#"{
            "symbol": "AAPL",
            "empty": true,
            "candles": []
        }"#;
        let resp: CandleList = serde_json::from_str(json).unwrap();
        assert!(resp.empty);
        assert!(resp.candles.is_empty());
        assert_eq!(resp.previous_close, None);
    }

    #[test]
    fn period_type_round_trips_known_variants() {
        for raw in ["day", "month", "year", "ytd"] {
            let json = format!(r#""{raw}""#);
            let parsed: PeriodType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn frequency_type_round_trips_known_variants() {
        for raw in ["minute", "daily", "weekly", "monthly"] {
            let json = format!(r#""{raw}""#);
            let parsed: FrequencyType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn unknown_period_type_preserves_raw_string() {
        let parsed: PeriodType = serde_json::from_str(r#""quarter""#).unwrap();
        assert!(matches!(parsed, PeriodType::Unknown(ref s) if s == "quarter"));
    }
}
