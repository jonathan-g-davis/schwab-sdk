//! `/quotes` and `/{symbol_id}/quotes` - snapshot quotes.
//!
//! The response is a map from symbol -> [`QuoteEntry`], dispatched on the
//! `assetMainType` field. Each documented asset class has its own typed
//! variant (equity, option, forex, future, future-option, index,
//! mutual-fund). Asset types Schwab adds later, and `BOND` (which has no
//! response schema), fall into [`QuoteEntry::Raw`]. Lookup failures
//! (invalid symbols / CUSIPs) come back as [`QuoteEntry::Error`].
//!
//! Reached through
//! [`MarketData::quotes`](super::MarketData::quotes).

use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use super::chains::{ExpirationType, SettlementType};
use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;

/// Accessor for the `/quotes` endpoint family. Construct via
/// [`MarketData::quotes`](super::MarketData::quotes).
pub struct Quotes<'a> {
    client: &'a SchwabClient,
}

impl<'a> Quotes<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Begin a `GET /quotes?symbols=...` batch request. Schwab will
    /// return a [`QuoteResponse`] map keyed by symbol; unknown symbols
    /// surface in the response as [`QuoteEntry::Error`] entries rather
    /// than failing the whole request.
    pub fn list<I, S>(&self, symbols: I) -> ListQuotesBuilder<'a>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let symbols = symbols
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect::<Vec<_>>()
            .join(",");
        ListQuotesBuilder {
            client: self.client,
            symbols,
            fields: None,
            indicative: None,
        }
    }

    /// Begin a `GET /{symbol}/quotes` single-symbol request. Useful when
    /// you want a quote for exactly one symbol and don't need the
    /// `indicative` flag (which is only on the batch endpoint).
    pub fn get(&self, symbol: impl Into<String>) -> GetQuoteBuilder<'a> {
        GetQuoteBuilder {
            client: self.client,
            symbol: symbol.into(),
            fields: None,
        }
    }
}

/// In-flight request for `GET /quotes`.
#[must_use = "call .send() to execute the request"]
pub struct ListQuotesBuilder<'a> {
    client: &'a SchwabClient,
    symbols: String,
    fields: Option<String>,
    indicative: Option<bool>,
}

impl<'a> ListQuotesBuilder<'a> {
    /// Restrict the response to a subset of root nodes. Defaults to
    /// returning every node (`fields=all`).
    pub fn fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = QuoteField>,
    {
        let csv = fields
            .into_iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");
        self.fields = Some(csv);
        self
    }

    /// Include indicative quotes for ETF symbols (e.g. `ABC` returns
    /// both `ABC` and `$ABC.IV`).
    pub fn indicative(mut self, value: bool) -> Self {
        self.indicative = Some(value);
        self
    }

    pub async fn send(self) -> Result<QuoteResponse> {
        let md = self.client.market_data_http();
        let mut request = md
            .get("/quotes")
            .query(&[("symbols", self.symbols.as_str())]);
        if let Some(fields) = &self.fields {
            request = request.query(&[("fields", fields.as_str())]);
        }
        if let Some(indicative) = self.indicative {
            let v = if indicative { "true" } else { "false" };
            request = request.query(&[("indicative", v)]);
        }
        md.execute_json(request).await
    }
}

/// In-flight request for `GET /{symbol}/quotes`.
#[must_use = "call .send() to execute the request"]
pub struct GetQuoteBuilder<'a> {
    client: &'a SchwabClient,
    symbol: String,
    fields: Option<String>,
}

impl<'a> GetQuoteBuilder<'a> {
    pub fn fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = QuoteField>,
    {
        let csv = fields
            .into_iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");
        self.fields = Some(csv);
        self
    }

    pub async fn send(self) -> Result<QuoteResponse> {
        let path = format!("/{}/quotes", self.symbol);
        let md = self.client.market_data_http();
        let mut request = md.get(&path);
        if let Some(fields) = &self.fields {
            request = request.query(&[("fields", fields.as_str())]);
        }
        md.execute_json(request).await
    }
}

// --- Response shape ---

/// Top-level response body for both `/quotes` and `/{symbol}/quotes`.
/// Schwab returns a map from symbol string to [`QuoteEntry`].
pub type QuoteResponse = HashMap<String, QuoteEntry>;

/// One per-symbol entry in a [`QuoteResponse`].
///
/// Dispatched on `assetMainType` at deserialize time:
/// - each documented `assetMainType` -> its typed variant (e.g. `EQUITY`
///   -> [`QuoteEntry::Equity`], `OPTION` -> [`QuoteEntry::Option`]).
/// - an `assetMainType` with no response schema (`BOND`) or one Schwab
///   adds after this crate was published -> [`QuoteEntry::Raw`] carrying
///   the original JSON for the consumer to inspect.
/// - Entries with no `assetMainType` (Schwab returns these when a
///   symbol was invalid) -> [`QuoteEntry::Error`] carrying the lists of
///   invalid symbols / cusips / SSIDs.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum QuoteEntry {
    Equity(Box<EquityQuote>),
    Option(Box<OptionQuote>),
    Forex(Box<ForexQuote>),
    Future(Box<FutureQuote>),
    FutureOption(Box<FutureOptionQuote>),
    Index(Box<IndexQuote>),
    MutualFund(Box<MutualFundQuote>),
    Error(QuoteError),
    /// An `assetMainType` with no response schema (`BOND`) or one Schwab
    /// adds after this crate was published. The consumer can inspect
    /// `assetMainType` and route on the raw value.
    Raw(serde_json::Value),
}

impl<'de> serde::Deserialize<'de> for QuoteEntry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Dispatch by `assetMainType`. The OpenAPI spec uses a oneOf
        // discriminator on this field; we look at the raw JSON value
        // once and route accordingly. Asset types Schwab adds later (and
        // `BOND`, which has no response schema) fall through to `Raw`.
        let value = serde_json::Value::deserialize(deserializer)?;
        macro_rules! typed {
            ($ty:ty, $variant:ident) => {{
                let q = <$ty>::deserialize(value).map_err(serde::de::Error::custom)?;
                Ok(QuoteEntry::$variant(Box::new(q)))
            }};
        }
        match value.get("assetMainType").and_then(|v| v.as_str()) {
            Some("EQUITY") => typed!(EquityQuote, Equity),
            Some("OPTION") => typed!(OptionQuote, Option),
            Some("FOREX") => typed!(ForexQuote, Forex),
            Some("FUTURE") => typed!(FutureQuote, Future),
            Some("FUTURE_OPTION") => typed!(FutureOptionQuote, FutureOption),
            Some("INDEX") => typed!(IndexQuote, Index),
            Some("MUTUAL_FUND") => typed!(MutualFundQuote, MutualFund),
            Some(_) => Ok(QuoteEntry::Raw(value)),
            None => {
                // No assetMainType - assume QuoteError shape.
                let e = QuoteError::deserialize(value).map_err(serde::de::Error::custom)?;
                Ok(QuoteEntry::Error(e))
            }
        }
    }
}

/// Equity-asset response: composes the `quote` / `reference` / `regular`
/// / `extended` / `fundamental` sub-objects with the asset metadata.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct EquityQuote {
    #[serde(rename = "assetMainType")]
    pub asset_main_type: AssetMainType,
    #[serde(rename = "assetSubType", default)]
    pub asset_sub_type: Option<AssetSubType>,
    #[serde(default)]
    pub ssid: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub realtime: Option<bool>,
    #[serde(rename = "quoteType", default)]
    pub quote_type: Option<QuoteType>,
    #[serde(default)]
    pub extended: Option<ExtendedMarket>,
    #[serde(default)]
    pub fundamental: Option<Fundamental>,
    #[serde(default)]
    pub quote: Option<QuoteEquity>,
    #[serde(default)]
    pub reference: Option<ReferenceEquity>,
    #[serde(default)]
    pub regular: Option<RegularMarket>,
}

/// Equity quote sub-object: bid/ask/last, day OHLC, mark, volume, etc.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteEquity {
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    #[serde(rename = "askMICId", default)]
    pub ask_mic_id: Option<String>,
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// Last ask time in epoch milliseconds.
    #[serde(rename = "askTime", default)]
    pub ask_time: Option<i64>,
    #[serde(rename = "bidMICId", default)]
    pub bid_mic_id: Option<String>,
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Last bid time in epoch milliseconds.
    #[serde(rename = "bidTime", default)]
    pub bid_time: Option<i64>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    #[serde(rename = "lastMICId", default)]
    pub last_mic_id: Option<String>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    #[serde(rename = "markPercentChange", default, with = "decimal_opt")]
    pub mark_percent_change: Option<Decimal>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub volatility: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ReferenceEquity {
    #[serde(default)]
    pub cusip: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    #[serde(rename = "fsiDesc", default)]
    pub fsi_desc: Option<String>,
    #[serde(rename = "htbQuantity", default)]
    pub htb_quantity: Option<i32>,
    #[serde(rename = "htbRate", default, with = "decimal_opt")]
    pub htb_rate: Option<Decimal>,
    #[serde(rename = "isHardToBorrow", default)]
    pub is_hard_to_borrow: Option<bool>,
    #[serde(rename = "isShortable", default)]
    pub is_shortable: Option<bool>,
    #[serde(rename = "otcMarketTier", default)]
    pub otc_market_tier: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct RegularMarket {
    #[serde(rename = "regularMarketLastPrice", default, with = "decimal_opt")]
    pub regular_market_last_price: Option<Decimal>,
    #[serde(rename = "regularMarketLastSize", default)]
    pub regular_market_last_size: Option<i32>,
    #[serde(rename = "regularMarketNetChange", default, with = "decimal_opt")]
    pub regular_market_net_change: Option<Decimal>,
    #[serde(rename = "regularMarketPercentChange", default, with = "decimal_opt")]
    pub regular_market_percent_change: Option<Decimal>,
    /// Epoch milliseconds.
    #[serde(rename = "regularMarketTradeTime", default)]
    pub regular_market_trade_time: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ExtendedMarket {
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Fundamental {
    #[serde(rename = "avg10DaysVolume", default, with = "decimal_opt")]
    pub avg_10_days_volume: Option<Decimal>,
    #[serde(rename = "avg1YearVolume", default, with = "decimal_opt")]
    pub avg_1_year_volume: Option<Decimal>,
    /// Schwab ships these dates as ISO-8601 strings (`yyyy-MM-ddTHH:mm:ssZ`).
    #[serde(rename = "declarationDate", default)]
    pub declaration_date: Option<String>,
    #[serde(rename = "divAmount", default, with = "decimal_opt")]
    pub div_amount: Option<Decimal>,
    #[serde(rename = "divExDate", default)]
    pub div_ex_date: Option<String>,
    /// Number of dividends per year (1 = annual, 4 = quarterly, etc.).
    #[serde(rename = "divFreq", default)]
    pub div_freq: Option<i32>,
    #[serde(rename = "divPayAmount", default, with = "decimal_opt")]
    pub div_pay_amount: Option<Decimal>,
    #[serde(rename = "divPayDate", default)]
    pub div_pay_date: Option<String>,
    #[serde(rename = "divYield", default, with = "decimal_opt")]
    pub div_yield: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub eps: Option<Decimal>,
    #[serde(rename = "fundLeverageFactor", default, with = "decimal_opt")]
    pub fund_leverage_factor: Option<Decimal>,
    #[serde(rename = "fundStrategy", default)]
    pub fund_strategy: Option<FundStrategy>,
    #[serde(rename = "nextDivExDate", default)]
    pub next_div_ex_date: Option<String>,
    #[serde(rename = "nextDivPayDate", default)]
    pub next_div_pay_date: Option<String>,
    #[serde(rename = "peRatio", default, with = "decimal_opt")]
    pub pe_ratio: Option<Decimal>,
}

/// Error block Schwab returns when one or more requested identifiers
/// could not be quoted.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteError {
    #[serde(rename = "invalidCusips", default)]
    pub invalid_cusips: Vec<String>,
    #[serde(rename = "invalidSSIDs", default)]
    pub invalid_ssids: Vec<i64>,
    #[serde(rename = "invalidSymbols", default)]
    pub invalid_symbols: Vec<String>,
}

// --- Option ---

/// Option-asset response: the `quote` / `reference` sub-objects with the
/// asset metadata.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OptionQuote {
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    #[serde(default)]
    pub ssid: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub realtime: Option<bool>,
    #[serde(default)]
    pub quote: Option<QuoteOption>,
    #[serde(default)]
    pub reference: Option<ReferenceOption>,
}

/// Option quote sub-object: bid/ask/last, the Greeks, and theoretical
/// values.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteOption {
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub delta: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub gamma: Option<Decimal>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Indicative ask price; only on indicative option symbols.
    #[serde(rename = "indAskPrice", default, with = "decimal_opt")]
    pub ind_ask_price: Option<Decimal>,
    /// Indicative bid price; only on indicative option symbols.
    #[serde(rename = "indBidPrice", default, with = "decimal_opt")]
    pub ind_bid_price: Option<Decimal>,
    /// Indicative quote time in epoch milliseconds; only on indicative
    /// option symbols.
    #[serde(rename = "indQuoteTime", default)]
    pub ind_quote_time: Option<i64>,
    #[serde(rename = "impliedYield", default, with = "decimal_opt")]
    pub implied_yield: Option<Decimal>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    #[serde(rename = "markPercentChange", default, with = "decimal_opt")]
    pub mark_percent_change: Option<Decimal>,
    #[serde(rename = "moneyIntrinsicValue", default, with = "decimal_opt")]
    pub money_intrinsic_value: Option<Decimal>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    #[serde(rename = "openInterest", default, with = "decimal_opt")]
    pub open_interest: Option<Decimal>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub rho: Option<Decimal>,
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    #[serde(rename = "theoreticalOptionValue", default, with = "decimal_opt")]
    pub theoretical_option_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub theta: Option<Decimal>,
    #[serde(rename = "timeValue", default, with = "decimal_opt")]
    pub time_value: Option<Decimal>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
    #[serde(rename = "underlyingPrice", default, with = "decimal_opt")]
    pub underlying_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub vega: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub volatility: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ReferenceOption {
    #[serde(rename = "contractType", default)]
    pub contract_type: Option<OptionContractType>,
    #[serde(default)]
    pub cusip: Option<String>,
    #[serde(rename = "daysToExpiration", default)]
    pub days_to_expiration: Option<i32>,
    /// Unit of trade description.
    #[serde(default)]
    pub deliverables: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    #[serde(rename = "exerciseType", default)]
    pub exercise_type: Option<ExerciseType>,
    #[serde(rename = "expirationDay", default)]
    pub expiration_day: Option<i32>,
    #[serde(rename = "expirationMonth", default)]
    pub expiration_month: Option<i32>,
    #[serde(rename = "expirationType", default)]
    pub expiration_type: Option<ExpirationType>,
    #[serde(rename = "expirationYear", default)]
    pub expiration_year: Option<i32>,
    #[serde(rename = "isPennyPilot", default)]
    pub is_penny_pilot: Option<bool>,
    /// Epoch milliseconds.
    #[serde(rename = "lastTradingDay", default)]
    pub last_trading_day: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    #[serde(rename = "settlementType", default)]
    pub settlement_type: Option<SettlementType>,
    #[serde(rename = "strikePrice", default, with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    #[serde(default)]
    pub underlying: Option<String>,
}

// --- Forex ---

/// Forex-asset response.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ForexQuote {
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    #[serde(default)]
    pub ssid: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub realtime: Option<bool>,
    #[serde(default)]
    pub quote: Option<QuoteForex>,
    #[serde(default)]
    pub reference: Option<ReferenceForex>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteForex {
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    #[serde(default, with = "decimal_opt")]
    pub tick: Option<Decimal>,
    #[serde(rename = "tickAmount", default, with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ReferenceForex {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    #[serde(rename = "isTradable", default)]
    pub is_tradable: Option<bool>,
    #[serde(rename = "marketMaker", default)]
    pub market_maker: Option<String>,
    #[serde(default)]
    pub product: Option<String>,
    #[serde(rename = "tradingHours", default)]
    pub trading_hours: Option<String>,
}

// --- Future ---

/// Future-asset response.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct FutureQuote {
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    #[serde(default)]
    pub ssid: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub realtime: Option<bool>,
    #[serde(default)]
    pub quote: Option<QuoteFuture>,
    #[serde(default)]
    pub reference: Option<ReferenceFuture>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteFuture {
    #[serde(rename = "askMICId", default)]
    pub ask_mic_id: Option<String>,
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// Last ask time in epoch milliseconds.
    #[serde(rename = "askTime", default)]
    pub ask_time: Option<i64>,
    #[serde(rename = "bidMICId", default)]
    pub bid_mic_id: Option<String>,
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Last bid time in epoch milliseconds.
    #[serde(rename = "bidTime", default)]
    pub bid_time: Option<i64>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    #[serde(rename = "futurePercentChange", default, with = "decimal_opt")]
    pub future_percent_change: Option<Decimal>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    #[serde(rename = "lastMICId", default)]
    pub last_mic_id: Option<String>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(rename = "openInterest", default)]
    pub open_interest: Option<i64>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    #[serde(rename = "quotedInSession", default)]
    pub quoted_in_session: Option<bool>,
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Settlement time in epoch milliseconds.
    #[serde(rename = "settleTime", default)]
    pub settle_time: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub tick: Option<Decimal>,
    #[serde(rename = "tickAmount", default, with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ReferenceFuture {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    #[serde(rename = "futureActiveSymbol", default)]
    pub future_active_symbol: Option<String>,
    /// Epoch milliseconds.
    #[serde(rename = "futureExpirationDate", default)]
    pub future_expiration_date: Option<i64>,
    #[serde(rename = "futureIsActive", default)]
    pub future_is_active: Option<bool>,
    #[serde(rename = "futureMultiplier", default, with = "decimal_opt")]
    pub future_multiplier: Option<Decimal>,
    #[serde(rename = "futurePriceFormat", default)]
    pub future_price_format: Option<String>,
    #[serde(rename = "futureSettlementPrice", default, with = "decimal_opt")]
    pub future_settlement_price: Option<Decimal>,
    #[serde(rename = "futureTradingHours", default)]
    pub future_trading_hours: Option<String>,
    #[serde(default)]
    pub product: Option<String>,
}

// --- Future option ---

/// Future-option-asset response.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct FutureOptionQuote {
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    #[serde(default)]
    pub ssid: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub realtime: Option<bool>,
    #[serde(default)]
    pub quote: Option<QuoteFutureOption>,
    #[serde(default)]
    pub reference: Option<ReferenceFutureOption>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteFutureOption {
    #[serde(rename = "askMICId", default)]
    pub ask_mic_id: Option<String>,
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    #[serde(rename = "bidMICId", default)]
    pub bid_mic_id: Option<String>,
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    #[serde(rename = "lastMICId", default)]
    pub last_mic_id: Option<String>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    #[serde(rename = "openInterest", default)]
    pub open_interest: Option<i64>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Settlement price. Schwab's published schema misspells the wire key
    /// as `settlemetPrice`; both spellings decode here.
    #[serde(
        rename = "settlemetPrice",
        alias = "settlementPrice",
        default,
        with = "decimal_opt"
    )]
    pub settlement_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub tick: Option<Decimal>,
    #[serde(rename = "tickAmount", default, with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ReferenceFutureOption {
    #[serde(rename = "contractType", default)]
    pub contract_type: Option<OptionContractType>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    #[serde(default, with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    /// Epoch milliseconds.
    #[serde(rename = "expirationDate", default)]
    pub expiration_date: Option<i64>,
    #[serde(rename = "expirationStyle", default)]
    pub expiration_style: Option<String>,
    #[serde(rename = "strikePrice", default, with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    #[serde(default)]
    pub underlying: Option<String>,
}

// --- Index ---

/// Index-asset response.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct IndexQuote {
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    #[serde(default)]
    pub ssid: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub realtime: Option<bool>,
    #[serde(default)]
    pub quote: Option<QuoteIndex>,
    #[serde(default)]
    pub reference: Option<ReferenceIndex>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteIndex {
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ReferenceIndex {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
}

// --- Mutual fund ---

/// Mutual-fund-asset response.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct MutualFundQuote {
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    #[serde(rename = "assetSubType", default)]
    pub asset_sub_type: Option<MutualFundAssetSubType>,
    #[serde(default)]
    pub ssid: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub realtime: Option<bool>,
    #[serde(default)]
    pub fundamental: Option<Fundamental>,
    #[serde(default)]
    pub quote: Option<QuoteMutualFund>,
    #[serde(default)]
    pub reference: Option<ReferenceMutualFund>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct QuoteMutualFund {
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Net asset value.
    #[serde(rename = "nAV", default, with = "decimal_opt")]
    pub nav: Option<Decimal>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct ReferenceMutualFund {
    #[serde(default)]
    pub cusip: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
}

// --- Enums ---

string_enum! {
    /// Asset class discriminator on a quote response.
    AssetMainType {
        Bond = "BOND",
        Equity = "EQUITY",
        Forex = "FOREX",
        Future = "FUTURE",
        FutureOption = "FUTURE_OPTION",
        Index = "INDEX",
        MutualFund = "MUTUAL_FUND",
        Option = "OPTION",
    }
}

string_enum! {
    /// Asset sub-type (only applicable to some asset classes).
    AssetSubType {
        Coe = "COE",
        Prf = "PRF",
        Adr = "ADR",
        Gdr = "GDR",
        Cef = "CEF",
        Etf = "ETF",
        Etn = "ETN",
        Uit = "UIT",
        War = "WAR",
        Rgt = "RGT",
    }
}

string_enum! {
    /// Quote freshness/source classification.
    QuoteType {
        /// National Best Bid and Offer; real-time.
        Nbbo = "NBBO",
        /// Non-fee-liable quote.
        Nfl = "NFL",
    }
}

string_enum! {
    /// Fund-strategy code: A=Active, L=Leveraged, P=Passive,
    /// Q=Quantitative, S=Short.
    FundStrategy {
        Active = "A",
        Leveraged = "L",
        Passive = "P",
        Quantitative = "Q",
        Short = "S",
    }
}

string_enum! {
    /// Asset sub-type for mutual-fund quotes.
    MutualFundAssetSubType {
        /// Open-end fund.
        Oef = "OEF",
        /// Closed-end fund.
        Cef = "CEF",
        /// Money-market fund.
        Mmf = "MMF",
    }
}

string_enum! {
    /// Call/put discriminator on an option or future-option reference.
    OptionContractType {
        Put = "P",
        Call = "C",
    }
}

string_enum! {
    /// Option exercise style.
    ExerciseType {
        /// American-style: exercisable any time before expiration.
        American = "A",
        /// European-style: exercisable only at expiration.
        European = "E",
    }
}

string_enum! {
    /// `fields` query parameter for the quote endpoints. Pass any
    /// combination via [`ListQuotesBuilder::fields`] /
    /// [`GetQuoteBuilder::fields`]; omitting the call defaults to `all`.
    QuoteField {
        Quote = "quote",
        Fundamental = "fundamental",
        Extended = "extended",
        Reference = "reference",
        Regular = "regular",
        All = "all",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn equity_quote_response_parses() {
        let json = r#"{
            "AAPL": {
                "assetMainType": "EQUITY",
                "assetSubType": "COE",
                "ssid": 1234567890,
                "symbol": "AAPL",
                "realtime": true,
                "quoteType": "NBBO",
                "quote": {
                    "52WeekHigh": 145.09,
                    "52WeekLow": 77.581,
                    "askPrice": 124.63,
                    "askSize": 700,
                    "askTime": 1621376892336,
                    "bidPrice": 124.6,
                    "bidSize": 300,
                    "bidTime": 1621376892336,
                    "closePrice": 126.27,
                    "highPrice": 126.99,
                    "lastPrice": 122.3,
                    "lastSize": 100,
                    "lowPrice": 122.0,
                    "mark": 122.3,
                    "netChange": -3.97,
                    "netPercentChange": -0.0314,
                    "openPrice": 126.0,
                    "quoteTime": 1621376892336,
                    "totalVolume": 20171188,
                    "tradeTime": 1621376731304
                },
                "reference": {
                    "cusip": "037833100",
                    "description": "Apple Inc. - Common Stock",
                    "exchange": "q",
                    "exchangeName": "NASDAQ",
                    "isShortable": true
                },
                "regular": {
                    "regularMarketLastPrice": 124.85,
                    "regularMarketTradeTime": 1621368000400
                }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 1);
        let entry = resp.get("AAPL").unwrap();
        let q = match entry {
            QuoteEntry::Equity(q) => q,
            other => panic!("expected Equity, got {other:?}"),
        };
        assert_eq!(q.asset_main_type, AssetMainType::Equity);
        assert_eq!(q.asset_sub_type, Some(AssetSubType::Coe));
        assert_eq!(q.ssid, Some(1234567890));
        assert_eq!(q.symbol.as_deref(), Some("AAPL"));
        assert_eq!(q.quote_type, Some(QuoteType::Nbbo));

        let quote = q.quote.as_ref().unwrap();
        assert_eq!(quote.ask_price, Some(dec!(124.63)));
        assert_eq!(quote.bid_price, Some(dec!(124.6)));
        assert_eq!(quote.last_price, Some(dec!(122.3)));
        assert_eq!(quote.week_52_high, Some(dec!(145.09)));
        assert_eq!(quote.total_volume, Some(20171188));
        assert_eq!(quote.ask_time, Some(1621376892336));

        let reference = q.reference.as_ref().unwrap();
        assert_eq!(reference.cusip.as_deref(), Some("037833100"));
        assert_eq!(reference.is_shortable, Some(true));

        let regular = q.regular.as_ref().unwrap();
        assert_eq!(regular.regular_market_last_price, Some(dec!(124.85)));
    }

    #[test]
    fn unschematized_asset_main_type_falls_back_to_raw() {
        // `BOND` is a documented assetMainType but the spec defines no
        // bond response schema, so it routes to Raw, as does any asset
        // type Schwab adds after this crate was published.
        let json = r#"{
            "912828YK0": {
                "assetMainType": "BOND",
                "symbol": "912828YK0",
                "quote": { "askPrice": 99.5 }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        let entry = resp.get("912828YK0").unwrap();
        let raw = match entry {
            QuoteEntry::Raw(v) => v,
            other => panic!("expected Raw, got {other:?}"),
        };
        assert_eq!(raw["assetMainType"], "BOND");
        assert_eq!(raw["quote"]["askPrice"], 99.5);
    }

    #[test]
    fn invalid_symbol_response_parses_as_error() {
        let json = r#"{
            "errors": {
                "invalidSymbols": ["BOGUS"],
                "invalidCusips": [],
                "invalidSSIDs": []
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        let entry = resp.get("errors").unwrap();
        let err = match entry {
            QuoteEntry::Error(e) => e,
            other => panic!("expected Error, got {other:?}"),
        };
        assert_eq!(err.invalid_symbols, vec!["BOGUS"]);
        assert!(err.invalid_cusips.is_empty());
    }

    #[test]
    fn mixed_equity_and_error_entries_parse() {
        let json = r#"{
            "AAPL": {
                "assetMainType": "EQUITY",
                "symbol": "AAPL",
                "quote": { "lastPrice": 122.3 }
            },
            "errors": { "invalidSymbols": ["BOGUS"] }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        assert!(matches!(resp.get("AAPL"), Some(QuoteEntry::Equity(_))));
        assert!(matches!(resp.get("errors"), Some(QuoteEntry::Error(_))));
    }

    #[test]
    fn quote_field_csv_emits_correct_wire_form() {
        // The fields() builder method joins on ',' via Display.
        let csv = [
            QuoteField::Quote,
            QuoteField::Reference,
            QuoteField::Regular,
        ]
        .into_iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");
        assert_eq!(csv, "quote,reference,regular");
    }

    #[test]
    fn unknown_asset_sub_type_preserves_raw_string() {
        let parsed: AssetSubType = serde_json::from_str(r#""NEW_SUB""#).unwrap();
        assert!(matches!(parsed, AssetSubType::Unknown(ref s) if s == "NEW_SUB"));
    }

    #[test]
    fn fundamental_with_dividend_data_parses() {
        let json = r#"{
            "avg10DaysVolume": 50000000.0,
            "divAmount": 0.88,
            "divExDate": "2021-05-07T00:00:00Z",
            "divFreq": 4,
            "divYield": 0.7,
            "eps": 4.45645,
            "fundStrategy": "P",
            "peRatio": 28.599
        }"#;
        let f: Fundamental = serde_json::from_str(json).unwrap();
        assert_eq!(f.div_amount, Some(dec!(0.88)));
        assert_eq!(f.div_freq, Some(4));
        assert_eq!(f.div_yield, Some(dec!(0.7)));
        assert_eq!(f.fund_strategy, Some(FundStrategy::Passive));
        assert_eq!(f.div_ex_date.as_deref(), Some("2021-05-07T00:00:00Z"));
    }

    #[test]
    fn option_quote_response_parses() {
        let json = r#"{
            "AMZN  220617C03170000": {
                "assetMainType": "OPTION",
                "symbol": "AMZN  220617C03170000",
                "ssid": 72507798,
                "realtime": true,
                "reference": {
                    "contractType": "C",
                    "daysToExpiration": 123,
                    "description": "Amazon.com Inc 06/17/2022 $3170 Call",
                    "exerciseType": "A",
                    "expirationDay": 17,
                    "expirationMonth": 6,
                    "expirationYear": 2022,
                    "expirationType": "S",
                    "isPennyPilot": true,
                    "lastTradingDay": 1655510400000,
                    "multiplier": 100,
                    "settlementType": "P",
                    "strikePrice": 3170,
                    "underlying": "AMZN"
                },
                "quote": {
                    "askPrice": 223,
                    "bidPrice": 217.65,
                    "delta": 0.5106,
                    "gamma": 0.0007,
                    "rho": 4.5173,
                    "theta": -0.9619,
                    "vega": 7.1633,
                    "openInterest": 0,
                    "underlyingPrice": 3129.205,
                    "volatility": 32.8918,
                    "totalVolume": 0
                }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        let q = match resp.get("AMZN  220617C03170000").unwrap() {
            QuoteEntry::Option(q) => q,
            other => panic!("expected Option, got {other:?}"),
        };
        assert_eq!(q.asset_main_type, Some(AssetMainType::Option));
        assert_eq!(q.ssid, Some(72507798));

        let quote = q.quote.as_ref().unwrap();
        assert_eq!(quote.delta, Some(dec!(0.5106)));
        assert_eq!(quote.gamma, Some(dec!(0.0007)));
        assert_eq!(quote.underlying_price, Some(dec!(3129.205)));
        assert_eq!(quote.open_interest, Some(dec!(0)));

        let reference = q.reference.as_ref().unwrap();
        assert_eq!(reference.contract_type, Some(OptionContractType::Call));
        assert_eq!(reference.exercise_type, Some(ExerciseType::American));
        assert_eq!(reference.expiration_type, Some(ExpirationType::Standard));
        assert_eq!(reference.settlement_type, Some(SettlementType::Pm));
        assert_eq!(reference.strike_price, Some(dec!(3170)));
    }

    #[test]
    fn forex_quote_response_parses() {
        let json = r#"{
            "EUR/USD": {
                "assetMainType": "FOREX",
                "symbol": "EUR/USD",
                "ssid": 1,
                "realtime": true,
                "reference": {
                    "description": "Euro/USDollar Spot",
                    "exchangeName": "GFT",
                    "isTradable": false,
                    "tradingHours": ""
                },
                "quote": {
                    "askPrice": 1.13456,
                    "bidPrice": 1.13434,
                    "lastPrice": 1.13445,
                    "tick": 0,
                    "tickAmount": 0
                }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        let q = match resp.get("EUR/USD").unwrap() {
            QuoteEntry::Forex(q) => q,
            other => panic!("expected Forex, got {other:?}"),
        };
        assert_eq!(q.quote.as_ref().unwrap().last_price, Some(dec!(1.13445)));
        assert_eq!(q.reference.as_ref().unwrap().is_tradable, Some(false));
    }

    #[test]
    fn future_quote_response_parses() {
        let json = r#"{
            "/ESZ21": {
                "assetMainType": "FUTURE",
                "symbol": "/ESZ21",
                "realtime": true,
                "reference": {
                    "description": "E-mini S&P 500 Index Futures,Dec-2021,ETH",
                    "futureActiveSymbol": "/ESZ21",
                    "futureExpirationDate": 1639717200000,
                    "futureIsActive": true,
                    "futureMultiplier": 50,
                    "futureSettlementPrice": 4696,
                    "product": "/ES"
                },
                "quote": {
                    "askPrice": 4694.5,
                    "askSize": 113,
                    "openInterest": 2328678,
                    "quotedInSession": false,
                    "tick": 0.25,
                    "tickAmount": 12.5,
                    "totalVolume": 550778
                }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        let q = match resp.get("/ESZ21").unwrap() {
            QuoteEntry::Future(q) => q,
            other => panic!("expected Future, got {other:?}"),
        };
        let quote = q.quote.as_ref().unwrap();
        assert_eq!(quote.open_interest, Some(2328678));
        assert_eq!(quote.tick_amount, Some(dec!(12.5)));
        assert_eq!(quote.quoted_in_session, Some(false));

        let reference = q.reference.as_ref().unwrap();
        assert_eq!(reference.future_expiration_date, Some(1639717200000));
        assert_eq!(reference.future_multiplier, Some(dec!(50)));
        assert_eq!(reference.product.as_deref(), Some("/ES"));
    }

    #[test]
    fn future_option_quote_response_parses() {
        // Exercises the `settlemetPrice` spec misspelling and its
        // `settlementPrice` alias.
        let misspelled = r#"{
            "./ESZ21C4000": {
                "assetMainType": "FUTURE_OPTION",
                "symbol": "./ESZ21C4000",
                "reference": { "contractType": "C", "underlying": "/ESZ21" },
                "quote": { "askPrice": 12.5, "settlemetPrice": 11.0, "openInterest": 42 }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(misspelled).unwrap();
        let q = match resp.get("./ESZ21C4000").unwrap() {
            QuoteEntry::FutureOption(q) => q,
            other => panic!("expected FutureOption, got {other:?}"),
        };
        assert_eq!(q.quote.as_ref().unwrap().settlement_price, Some(dec!(11.0)));
        assert_eq!(q.quote.as_ref().unwrap().open_interest, Some(42));
        assert_eq!(
            q.reference.as_ref().unwrap().contract_type,
            Some(OptionContractType::Call)
        );

        let aliased = r#"{
            "./ESZ21C4000": {
                "assetMainType": "FUTURE_OPTION",
                "quote": { "settlementPrice": 11.0 }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(aliased).unwrap();
        let q = match resp.get("./ESZ21C4000").unwrap() {
            QuoteEntry::FutureOption(q) => q,
            other => panic!("expected FutureOption, got {other:?}"),
        };
        assert_eq!(q.quote.as_ref().unwrap().settlement_price, Some(dec!(11.0)));
    }

    #[test]
    fn index_quote_response_parses() {
        let json = r#"{
            "$SPX": {
                "assetMainType": "INDEX",
                "symbol": "$SPX",
                "ssid": 1819771877,
                "realtime": true,
                "reference": {
                    "description": "S&P 500 Index",
                    "exchangeName": "Index"
                },
                "quote": {
                    "52WeekHigh": 4423.46,
                    "lastPrice": 4396.2,
                    "netChange": -369.98,
                    "totalVolume": 628009977
                }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        let q = match resp.get("$SPX").unwrap() {
            QuoteEntry::Index(q) => q,
            other => panic!("expected Index, got {other:?}"),
        };
        let quote = q.quote.as_ref().unwrap();
        assert_eq!(quote.last_price, Some(dec!(4396.2)));
        assert_eq!(quote.total_volume, Some(628009977));
    }

    #[test]
    fn mutual_fund_quote_response_parses() {
        let json = r#"{
            "AAAIX": {
                "assetMainType": "MUTUAL_FUND",
                "assetSubType": "OEF",
                "symbol": "AAAIX",
                "realtime": true,
                "reference": {
                    "cusip": "025085853",
                    "description": "American Century Strategic Allocation: Aggressive Fund - I Class",
                    "exchangeName": "Mutual Fund"
                },
                "quote": {
                    "52WeekHigh": 9.24,
                    "closePrice": 9.12,
                    "nAV": 0,
                    "netChange": -0.03
                },
                "fundamental": { "divYield": 0.83059 }
            }
        }"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        let q = match resp.get("AAAIX").unwrap() {
            QuoteEntry::MutualFund(q) => q,
            other => panic!("expected MutualFund, got {other:?}"),
        };
        assert_eq!(q.asset_sub_type, Some(MutualFundAssetSubType::Oef));
        assert_eq!(q.quote.as_ref().unwrap().nav, Some(dec!(0)));
        assert_eq!(q.quote.as_ref().unwrap().close_price, Some(dec!(9.12)));
        assert_eq!(
            q.fundamental.as_ref().unwrap().div_yield,
            Some(dec!(0.83059))
        );
    }

    #[test]
    fn option_contract_type_round_trips_single_letter_codes() {
        for raw in ["P", "C"] {
            let json = format!(r#""{raw}""#);
            let parsed: OptionContractType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn exercise_type_round_trips_single_letter_codes() {
        for raw in ["A", "E"] {
            let json = format!(r#""{raw}""#);
            let parsed: ExerciseType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn mutual_fund_asset_sub_type_round_trips_known_variants() {
        for raw in ["OEF", "CEF", "MMF"] {
            let json = format!(r#""{raw}""#);
            let parsed: MutualFundAssetSubType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }
}
