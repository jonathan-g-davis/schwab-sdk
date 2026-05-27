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
//!
//! # Examples
//!
//! Without [`ListQuotesBuilder::fields`] Schwab returns every root node;
//! narrowing the selection reduces the payload. The response is a map keyed
//! by symbol; match on the [`QuoteEntry`] variant to read the typed fields.
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient};
//! use schwab_sdk::market_data::{QuoteEntry, QuoteField};
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! let quotes = client
//!     .market_data()
//!     .quotes()
//!     .list(["AAPL"])
//!     .fields([QuoteField::Quote, QuoteField::Reference])
//!     .send()
//!     .await?;
//!
//! if let Some(QuoteEntry::Equity(equity)) = quotes.get("AAPL") {
//!     if let Some(quote) = &equity.quote {
//!         println!("bid {:?} / ask {:?}", quote.bid_price, quote.ask_price);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

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
#[derive(Debug)]
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
#[derive(Debug)]
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

    /// Execute the request.
    pub async fn send(self) -> Result<QuoteResponse> {
        let mut request = self
            .client
            .market_data_http()
            .get("/quotes")
            .query(&[("symbols", self.symbols.as_str())]);
        if let Some(fields) = &self.fields {
            request = request.query(&[("fields", fields.as_str())]);
        }
        if let Some(indicative) = self.indicative {
            let v = if indicative { "true" } else { "false" };
            request = request.query(&[("indicative", v)]);
        }
        request.send_json().await
    }
}

/// In-flight request for `GET /{symbol}/quotes`.
#[derive(Debug)]
#[must_use = "call .send() to execute the request"]
pub struct GetQuoteBuilder<'a> {
    client: &'a SchwabClient,
    symbol: String,
    fields: Option<String>,
}

impl<'a> GetQuoteBuilder<'a> {
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

    /// Execute the request.
    pub async fn send(self) -> Result<QuoteResponse> {
        let path = format!("/{}/quotes", self.symbol);
        let mut request = self.client.market_data_http().get(&path);
        if let Some(fields) = &self.fields {
            request = request.query(&[("fields", fields.as_str())]);
        }
        request.send_json().await
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum QuoteEntry {
    /// Equity (`assetMainType` = `EQUITY`).
    Equity(Box<EquityQuote>),
    /// Listed option (`assetMainType` = `OPTION`).
    Option(Box<OptionQuote>),
    /// Forex pair (`assetMainType` = `FOREX`).
    Forex(Box<ForexQuote>),
    /// Futures contract (`assetMainType` = `FUTURE`).
    Future(Box<FutureQuote>),
    /// Futures option (`assetMainType` = `FUTURE_OPTION`).
    FutureOption(Box<FutureOptionQuote>),
    /// Index (`assetMainType` = `INDEX`).
    Index(Box<IndexQuote>),
    /// Mutual fund (`assetMainType` = `MUTUAL_FUND`).
    MutualFund(Box<MutualFundQuote>),
    /// Lookup failure (the entry's symbol / CUSIP / SSID was invalid).
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
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EquityQuote {
    /// Asset class discriminator (always [`AssetMainType::Equity`]).
    #[serde(rename = "assetMainType")]
    pub asset_main_type: AssetMainType,
    /// Sub-type (e.g. ETF, ADR, common equity).
    #[serde(rename = "assetSubType", default)]
    pub asset_sub_type: Option<AssetSubType>,
    /// Schwab security id.
    #[serde(default)]
    pub ssid: Option<i64>,
    /// Wire symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// `true` if the quote is real-time, `false` for delayed.
    #[serde(default)]
    pub realtime: Option<bool>,
    /// Quote source / freshness classification.
    #[serde(rename = "quoteType", default)]
    pub quote_type: Option<QuoteType>,
    /// Pre- / post-market quote block.
    #[serde(default)]
    pub extended: Option<ExtendedMarket>,
    /// Fundamental data (dividends, P/E, volume averages, etc.).
    #[serde(default)]
    pub fundamental: Option<Fundamental>,
    /// Live bid/ask/last quote block.
    #[serde(default)]
    pub quote: Option<QuoteEquity>,
    /// Static reference data (CUSIP, exchange, shortability).
    #[serde(default)]
    pub reference: Option<ReferenceEquity>,
    /// Last regular-session trade summary.
    #[serde(default)]
    pub regular: Option<RegularMarket>,
}

/// Equity quote sub-object: bid/ask/last, day OHLC, mark, volume, etc.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteEquity {
    /// 52-week high price, USD.
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    /// 52-week low price, USD.
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    /// MIC venue id for the best ask.
    #[serde(rename = "askMICId", default)]
    pub ask_mic_id: Option<String>,
    /// Best ask price, USD.
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Best ask size (shares).
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// Last ask time in epoch milliseconds.
    #[serde(rename = "askTime", default)]
    pub ask_time: Option<i64>,
    /// MIC venue id for the best bid.
    #[serde(rename = "bidMICId", default)]
    pub bid_mic_id: Option<String>,
    /// Best bid price, USD.
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Best bid size (shares).
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Last bid time in epoch milliseconds.
    #[serde(rename = "bidTime", default)]
    pub bid_time: Option<i64>,
    /// Prior session close price, USD.
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Day high, USD.
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// MIC venue id for the last trade.
    #[serde(rename = "lastMICId", default)]
    pub last_mic_id: Option<String>,
    /// Last trade price, USD.
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Last trade size (shares).
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    /// Day low, USD.
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Mark price (mid / Schwab-computed reference), USD.
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Mark change since prior close, USD.
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    /// Mark change since prior close as a fraction.
    #[serde(rename = "markPercentChange", default, with = "decimal_opt")]
    pub mark_percent_change: Option<Decimal>,
    /// Net change since prior close (last - close), USD.
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Net change since prior close as a fraction.
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Day open, USD.
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    /// Security status (e.g. `"Normal"`, `"Halted"`).
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Cumulative session volume (shares).
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
    /// Implied volatility (where Schwab supplies one for the equity).
    #[serde(default, with = "decimal_opt")]
    pub volatility: Option<Decimal>,
}

/// Static reference data for an equity quote.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReferenceEquity {
    /// CUSIP.
    #[serde(default)]
    pub cusip: Option<String>,
    /// Issuer description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab exchange code (single letter, e.g. `"q"` for NASDAQ).
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    /// Financial Status Indicator description.
    #[serde(rename = "fsiDesc", default)]
    pub fsi_desc: Option<String>,
    /// Hard-to-borrow quantity (shares available to short).
    #[serde(rename = "htbQuantity", default)]
    pub htb_quantity: Option<i32>,
    /// Hard-to-borrow rate (annualized).
    #[serde(rename = "htbRate", default, with = "decimal_opt")]
    pub htb_rate: Option<Decimal>,
    /// `true` if the equity is hard-to-borrow.
    #[serde(rename = "isHardToBorrow", default)]
    pub is_hard_to_borrow: Option<bool>,
    /// `true` if the equity is shortable.
    #[serde(rename = "isShortable", default)]
    pub is_shortable: Option<bool>,
    /// OTC market tier classification (when applicable).
    #[serde(rename = "otcMarketTier", default)]
    pub otc_market_tier: Option<String>,
}

/// Last regular-session trade summary.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct RegularMarket {
    /// Last regular-session trade price, USD.
    #[serde(rename = "regularMarketLastPrice", default, with = "decimal_opt")]
    pub regular_market_last_price: Option<Decimal>,
    /// Last regular-session trade size (shares).
    #[serde(rename = "regularMarketLastSize", default)]
    pub regular_market_last_size: Option<i32>,
    /// Net change since prior close, USD.
    #[serde(rename = "regularMarketNetChange", default, with = "decimal_opt")]
    pub regular_market_net_change: Option<Decimal>,
    /// Net change since prior close as a fraction.
    #[serde(rename = "regularMarketPercentChange", default, with = "decimal_opt")]
    pub regular_market_percent_change: Option<Decimal>,
    /// Last regular-session trade time, epoch milliseconds.
    #[serde(rename = "regularMarketTradeTime", default)]
    pub regular_market_trade_time: Option<i64>,
}

/// Pre-/post-market quote block.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ExtendedMarket {
    /// Extended-hours best ask, USD.
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Extended-hours best ask size.
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// Extended-hours best bid, USD.
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Extended-hours best bid size.
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Extended-hours last trade price, USD.
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Extended-hours last trade size.
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    /// Extended-hours mark price, USD.
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Extended-hours last quote time, epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    /// Extended-hours cumulative volume.
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Extended-hours last trade time, epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

/// Fundamental data block returned with equity and mutual-fund quotes.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Fundamental {
    /// Trailing 10-day average daily volume (shares).
    #[serde(rename = "avg10DaysVolume", default, with = "decimal_opt")]
    pub avg_10_days_volume: Option<Decimal>,
    /// Trailing 1-year average daily volume (shares).
    #[serde(rename = "avg1YearVolume", default, with = "decimal_opt")]
    pub avg_1_year_volume: Option<Decimal>,
    /// Dividend declaration date. Schwab ships these dates as ISO-8601
    /// strings (`yyyy-MM-ddTHH:mm:ssZ`).
    #[serde(rename = "declarationDate", default)]
    pub declaration_date: Option<String>,
    /// Most recent dividend amount, USD per share.
    #[serde(rename = "divAmount", default, with = "decimal_opt")]
    pub div_amount: Option<Decimal>,
    /// Most recent dividend ex-date (ISO-8601 string).
    #[serde(rename = "divExDate", default)]
    pub div_ex_date: Option<String>,
    /// Number of dividends per year (1 = annual, 4 = quarterly, etc.).
    #[serde(rename = "divFreq", default)]
    pub div_freq: Option<i32>,
    /// Most recent dividend pay amount, USD per share.
    #[serde(rename = "divPayAmount", default, with = "decimal_opt")]
    pub div_pay_amount: Option<Decimal>,
    /// Most recent dividend pay date (ISO-8601 string).
    #[serde(rename = "divPayDate", default)]
    pub div_pay_date: Option<String>,
    /// Trailing dividend yield as a fraction.
    #[serde(rename = "divYield", default, with = "decimal_opt")]
    pub div_yield: Option<Decimal>,
    /// Trailing earnings per share, USD.
    #[serde(default, with = "decimal_opt")]
    pub eps: Option<Decimal>,
    /// Leverage factor for leveraged funds (e.g. 3.0 for a 3x fund).
    #[serde(rename = "fundLeverageFactor", default, with = "decimal_opt")]
    pub fund_leverage_factor: Option<Decimal>,
    /// Fund strategy classification (active/leveraged/passive/...).
    #[serde(rename = "fundStrategy", default)]
    pub fund_strategy: Option<FundStrategy>,
    /// Next projected dividend ex-date (ISO-8601 string).
    #[serde(rename = "nextDivExDate", default)]
    pub next_div_ex_date: Option<String>,
    /// Next projected dividend pay date (ISO-8601 string).
    #[serde(rename = "nextDivPayDate", default)]
    pub next_div_pay_date: Option<String>,
    /// Trailing price-to-earnings ratio.
    #[serde(rename = "peRatio", default, with = "decimal_opt")]
    pub pe_ratio: Option<Decimal>,
}

/// Error block Schwab returns when one or more requested identifiers
/// could not be quoted.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteError {
    /// CUSIPs Schwab could not resolve.
    #[serde(rename = "invalidCusips", default)]
    pub invalid_cusips: Vec<String>,
    /// SSIDs Schwab could not resolve.
    #[serde(rename = "invalidSSIDs", default)]
    pub invalid_ssids: Vec<i64>,
    /// Symbols Schwab could not resolve.
    #[serde(rename = "invalidSymbols", default)]
    pub invalid_symbols: Vec<String>,
}

// --- Option ---

/// Option-asset response: the `quote`/`reference` sub-objects with the
/// asset metadata.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OptionQuote {
    /// Asset class discriminator.
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    /// Schwab security id.
    #[serde(default)]
    pub ssid: Option<i64>,
    /// Wire symbol (Schwab OSI format, e.g. `"AAPL  240315C00200000"`).
    #[serde(default)]
    pub symbol: Option<String>,
    /// `true` if the quote is real-time.
    #[serde(default)]
    pub realtime: Option<bool>,
    /// Live quote block (bid/ask/last + Greeks).
    #[serde(default)]
    pub quote: Option<QuoteOption>,
    /// Static reference data (strike, expiration, deliverables).
    #[serde(default)]
    pub reference: Option<ReferenceOption>,
}

/// Option quote sub-object: bid/ask/last, the Greeks, and theoretical
/// values.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteOption {
    /// 52-week high price, USD.
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    /// 52-week low price, USD.
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    /// Best ask premium, USD.
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Best ask size (contracts).
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// Best bid premium, USD.
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Best bid size (contracts).
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Prior session close premium, USD.
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Delta (Black-Scholes).
    #[serde(default, with = "decimal_opt")]
    pub delta: Option<Decimal>,
    /// Gamma (Black-Scholes).
    #[serde(default, with = "decimal_opt")]
    pub gamma: Option<Decimal>,
    /// Day high premium, USD.
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
    /// Implied yield (where Schwab supplies one).
    #[serde(rename = "impliedYield", default, with = "decimal_opt")]
    pub implied_yield: Option<Decimal>,
    /// Last trade premium, USD.
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Last trade size (contracts).
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    /// Day low premium, USD.
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Mark price (mid/Schwab-computed reference), USD.
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Mark change since prior close, USD.
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    /// Mark change since prior close as a fraction.
    #[serde(rename = "markPercentChange", default, with = "decimal_opt")]
    pub mark_percent_change: Option<Decimal>,
    /// In-the-money portion of the premium, USD.
    #[serde(rename = "moneyIntrinsicValue", default, with = "decimal_opt")]
    pub money_intrinsic_value: Option<Decimal>,
    /// Net change since prior close, USD.
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Net change since prior close as a fraction.
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Open interest (contracts).
    #[serde(rename = "openInterest", default, with = "decimal_opt")]
    pub open_interest: Option<Decimal>,
    /// Day open premium, USD.
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    /// Rho (Black-Scholes).
    #[serde(default, with = "decimal_opt")]
    pub rho: Option<Decimal>,
    /// Security status (e.g. `"Normal"`, `"Halted"`).
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Theoretical fair value from Schwab's pricing model, USD.
    #[serde(rename = "theoreticalOptionValue", default, with = "decimal_opt")]
    pub theoretical_option_value: Option<Decimal>,
    /// Theta (Black-Scholes).
    #[serde(default, with = "decimal_opt")]
    pub theta: Option<Decimal>,
    /// Extrinsic (time) value, USD.
    #[serde(rename = "timeValue", default, with = "decimal_opt")]
    pub time_value: Option<Decimal>,
    /// Cumulative session volume (contracts).
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
    /// Underlying price used by Schwab's pricing model, USD.
    #[serde(rename = "underlyingPrice", default, with = "decimal_opt")]
    pub underlying_price: Option<Decimal>,
    /// Vega (Black-Scholes).
    #[serde(default, with = "decimal_opt")]
    pub vega: Option<Decimal>,
    /// Implied volatility as a percentage.
    #[serde(default, with = "decimal_opt")]
    pub volatility: Option<Decimal>,
}

/// Static reference data for an option quote.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReferenceOption {
    /// Put/call discriminator.
    #[serde(rename = "contractType", default)]
    pub contract_type: Option<OptionContractType>,
    /// CUSIP of the option contract.
    #[serde(default)]
    pub cusip: Option<String>,
    /// Calendar days until expiration.
    #[serde(rename = "daysToExpiration", default)]
    pub days_to_expiration: Option<i32>,
    /// Unit of trade description.
    #[serde(default)]
    pub deliverables: Option<String>,
    /// Human-readable contract description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab exchange code.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    /// American/European exercise style.
    #[serde(rename = "exerciseType", default)]
    pub exercise_type: Option<ExerciseType>,
    /// Day-of-month of expiration.
    #[serde(rename = "expirationDay", default)]
    pub expiration_day: Option<i32>,
    /// Month of expiration (1-12).
    #[serde(rename = "expirationMonth", default)]
    pub expiration_month: Option<i32>,
    /// Expiration classification (standard/weekly/quarterly/...).
    #[serde(rename = "expirationType", default)]
    pub expiration_type: Option<ExpirationType>,
    /// Year of expiration.
    #[serde(rename = "expirationYear", default)]
    pub expiration_year: Option<i32>,
    /// `true` if the contract is in the SEC Penny Pilot program.
    #[serde(rename = "isPennyPilot", default)]
    pub is_penny_pilot: Option<bool>,
    /// Last trading day, epoch milliseconds.
    #[serde(rename = "lastTradingDay", default)]
    pub last_trading_day: Option<i64>,
    /// Shares-per-contract multiplier (typically 100).
    #[serde(default, with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    /// Settlement classification (AM/PM).
    #[serde(rename = "settlementType", default)]
    pub settlement_type: Option<SettlementType>,
    /// Strike price, USD.
    #[serde(rename = "strikePrice", default, with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    /// Symbol of the underlying instrument.
    #[serde(default)]
    pub underlying: Option<String>,
}

// --- Forex ---

/// Forex-asset response.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ForexQuote {
    /// Asset class discriminator.
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    /// Schwab security id.
    #[serde(default)]
    pub ssid: Option<i64>,
    /// Wire symbol (e.g. `"EUR/USD"`).
    #[serde(default)]
    pub symbol: Option<String>,
    /// `true` if the quote is real-time.
    #[serde(default)]
    pub realtime: Option<bool>,
    /// Live quote block.
    #[serde(default)]
    pub quote: Option<QuoteForex>,
    /// Static reference data for the pair.
    #[serde(default)]
    pub reference: Option<ReferenceForex>,
}

/// Live forex quote block.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteForex {
    /// 52-week high (counter-currency units per base unit).
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    /// 52-week low.
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    /// Best ask price.
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Best ask size.
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// Best bid price.
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Best bid size.
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Prior session close.
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Day high.
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Last trade price.
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Last trade size.
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    /// Day low.
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Mark price.
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Net change since prior close.
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Net change since prior close as a fraction.
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Day open.
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    /// Security status string.
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Minimum tick size.
    #[serde(default, with = "decimal_opt")]
    pub tick: Option<Decimal>,
    /// Notional value of one tick.
    #[serde(rename = "tickAmount", default, with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    /// Cumulative session volume.
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

/// Static reference data for a forex quote.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReferenceForex {
    /// Pair description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab exchange code.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    /// `true` if the pair is tradable through Schwab.
    #[serde(rename = "isTradable", default)]
    pub is_tradable: Option<bool>,
    /// Market maker name (when applicable).
    #[serde(rename = "marketMaker", default)]
    pub market_maker: Option<String>,
    /// Product/instrument category.
    #[serde(default)]
    pub product: Option<String>,
    /// Trading-hours description.
    #[serde(rename = "tradingHours", default)]
    pub trading_hours: Option<String>,
}

// --- Future ---

/// Future-asset response.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct FutureQuote {
    /// Asset class discriminator.
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    /// Schwab security id.
    #[serde(default)]
    pub ssid: Option<i64>,
    /// Wire symbol (Schwab CME format, e.g. `"/ESH24"`).
    #[serde(default)]
    pub symbol: Option<String>,
    /// `true` if the quote is real-time.
    #[serde(default)]
    pub realtime: Option<bool>,
    /// Live quote block.
    #[serde(default)]
    pub quote: Option<QuoteFuture>,
    /// Static reference data for the contract.
    #[serde(default)]
    pub reference: Option<ReferenceFuture>,
}

/// Live futures quote block.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteFuture {
    /// MIC venue id for the best ask.
    #[serde(rename = "askMICId", default)]
    pub ask_mic_id: Option<String>,
    /// Best ask price.
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Best ask size (contracts).
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// Last ask time in epoch milliseconds.
    #[serde(rename = "askTime", default)]
    pub ask_time: Option<i64>,
    /// MIC venue id for the best bid.
    #[serde(rename = "bidMICId", default)]
    pub bid_mic_id: Option<String>,
    /// Best bid price.
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Best bid size (contracts).
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Last bid time in epoch milliseconds.
    #[serde(rename = "bidTime", default)]
    pub bid_time: Option<i64>,
    /// Prior session settlement/close price.
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Session price change as a fraction (futures-specific).
    #[serde(rename = "futurePercentChange", default, with = "decimal_opt")]
    pub future_percent_change: Option<Decimal>,
    /// Day high price.
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// MIC venue id for the last trade.
    #[serde(rename = "lastMICId", default)]
    pub last_mic_id: Option<String>,
    /// Last trade price.
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Last trade size (contracts).
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    /// Day low price.
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Mark price.
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Net change since prior close.
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Open interest (contracts).
    #[serde(rename = "openInterest", default)]
    pub open_interest: Option<i64>,
    /// Day open price.
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    /// `true` if the quote was sampled during a regular session.
    #[serde(rename = "quotedInSession", default)]
    pub quoted_in_session: Option<bool>,
    /// Security status string.
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Settlement time in epoch milliseconds.
    #[serde(rename = "settleTime", default)]
    pub settle_time: Option<i64>,
    /// Minimum tick size.
    #[serde(default, with = "decimal_opt")]
    pub tick: Option<Decimal>,
    /// Notional value of one tick, USD.
    #[serde(rename = "tickAmount", default, with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    /// Cumulative session volume (contracts).
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

/// Static reference data for a futures quote.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReferenceFuture {
    /// Contract description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab exchange code.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    /// Active (front-month) symbol for this product.
    #[serde(rename = "futureActiveSymbol", default)]
    pub future_active_symbol: Option<String>,
    /// Expiration date in epoch milliseconds.
    #[serde(rename = "futureExpirationDate", default)]
    pub future_expiration_date: Option<i64>,
    /// `true` if this contract is the front month.
    #[serde(rename = "futureIsActive", default)]
    pub future_is_active: Option<bool>,
    /// Contract multiplier (USD per point).
    #[serde(rename = "futureMultiplier", default, with = "decimal_opt")]
    pub future_multiplier: Option<Decimal>,
    /// Schwab price-format string.
    #[serde(rename = "futurePriceFormat", default)]
    pub future_price_format: Option<String>,
    /// Settlement price, USD.
    #[serde(rename = "futureSettlementPrice", default, with = "decimal_opt")]
    pub future_settlement_price: Option<Decimal>,
    /// Trading-hours description.
    #[serde(rename = "futureTradingHours", default)]
    pub future_trading_hours: Option<String>,
    /// Product/root description.
    #[serde(default)]
    pub product: Option<String>,
}

// --- Future option ---

/// Future-option-asset response.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct FutureOptionQuote {
    /// Asset class discriminator.
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    /// Schwab security id.
    #[serde(default)]
    pub ssid: Option<i64>,
    /// Wire symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// `true` if the quote is real-time.
    #[serde(default)]
    pub realtime: Option<bool>,
    /// Live quote block.
    #[serde(default)]
    pub quote: Option<QuoteFutureOption>,
    /// Static reference data for the contract.
    #[serde(default)]
    pub reference: Option<ReferenceFutureOption>,
}

/// Live futures-option quote block.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteFutureOption {
    /// MIC venue id for the best ask.
    #[serde(rename = "askMICId", default)]
    pub ask_mic_id: Option<String>,
    /// Best ask premium.
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    /// Best ask size (contracts).
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i32>,
    /// MIC venue id for the best bid.
    #[serde(rename = "bidMICId", default)]
    pub bid_mic_id: Option<String>,
    /// Best bid premium.
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    /// Best bid size (contracts).
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i32>,
    /// Prior session close premium.
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Day high premium.
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// MIC venue id for the last trade.
    #[serde(rename = "lastMICId", default)]
    pub last_mic_id: Option<String>,
    /// Last trade premium.
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Last trade size (contracts).
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i32>,
    /// Day low premium.
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Mark price.
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    /// Mark change since prior close.
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    /// Net change since prior close.
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Net change since prior close as a fraction.
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Open interest (contracts).
    #[serde(rename = "openInterest", default)]
    pub open_interest: Option<i64>,
    /// Day open premium.
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Last quote time in epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    /// Security status string.
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
    /// Minimum tick size.
    #[serde(default, with = "decimal_opt")]
    pub tick: Option<Decimal>,
    /// Notional value of one tick, USD.
    #[serde(rename = "tickAmount", default, with = "decimal_opt")]
    pub tick_amount: Option<Decimal>,
    /// Cumulative session volume (contracts).
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

/// Static reference data for a futures-option quote.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReferenceFutureOption {
    /// Put/call discriminator.
    #[serde(rename = "contractType", default)]
    pub contract_type: Option<OptionContractType>,
    /// Contract description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab exchange code.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    /// Shares/units per contract multiplier.
    #[serde(default, with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    /// Expiration date in epoch milliseconds.
    #[serde(rename = "expirationDate", default)]
    pub expiration_date: Option<i64>,
    /// Expiration style description (American/European/...).
    #[serde(rename = "expirationStyle", default)]
    pub expiration_style: Option<String>,
    /// Strike price.
    #[serde(rename = "strikePrice", default, with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    /// Symbol of the underlying futures contract.
    #[serde(default)]
    pub underlying: Option<String>,
}

// --- Index ---

/// Index-asset response.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct IndexQuote {
    /// Asset class discriminator.
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    /// Schwab security id.
    #[serde(default)]
    pub ssid: Option<i64>,
    /// Wire symbol (e.g. `"$SPX"`).
    #[serde(default)]
    pub symbol: Option<String>,
    /// `true` if the quote is real-time.
    #[serde(default)]
    pub realtime: Option<bool>,
    /// Live quote block (no bid/ask; indices are non-tradeable).
    #[serde(default)]
    pub quote: Option<QuoteIndex>,
    /// Static reference data.
    #[serde(default)]
    pub reference: Option<ReferenceIndex>,
}

/// Live index quote block. Indices have no bid/ask; only last-price-style
/// fields.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteIndex {
    /// 52-week high.
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    /// 52-week low.
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    /// Prior session close.
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Day high.
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    /// Last value.
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    /// Day low.
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    /// Net change since prior close.
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// Net change since prior close as a fraction.
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Day open.
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    /// Security status string.
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Cumulative session volume of constituent trades.
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

/// Static reference data for an index quote.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReferenceIndex {
    /// Index description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab exchange code.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
}

// --- Mutual fund ---

/// Mutual-fund-asset response.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MutualFundQuote {
    /// Asset class discriminator.
    #[serde(rename = "assetMainType", default)]
    pub asset_main_type: Option<AssetMainType>,
    /// Fund sub-type (open-end/closed-end/money-market).
    #[serde(rename = "assetSubType", default)]
    pub asset_sub_type: Option<MutualFundAssetSubType>,
    /// Schwab security id.
    #[serde(default)]
    pub ssid: Option<i64>,
    /// Wire symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// `true` if the quote is real-time.
    #[serde(default)]
    pub realtime: Option<bool>,
    /// Fundamental data (yields, expense ratio, etc.).
    #[serde(default)]
    pub fundamental: Option<Fundamental>,
    /// Live quote block.
    #[serde(default)]
    pub quote: Option<QuoteMutualFund>,
    /// Static reference data.
    #[serde(default)]
    pub reference: Option<ReferenceMutualFund>,
}

/// Live mutual-fund quote block. Mutual funds price once per day.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QuoteMutualFund {
    /// 52-week high NAV, USD.
    #[serde(rename = "52WeekHigh", default, with = "decimal_opt")]
    pub week_52_high: Option<Decimal>,
    /// 52-week low NAV, USD.
    #[serde(rename = "52WeekLow", default, with = "decimal_opt")]
    pub week_52_low: Option<Decimal>,
    /// Prior session close NAV, USD.
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    /// Net asset value, USD.
    #[serde(rename = "nAV", default, with = "decimal_opt")]
    pub nav: Option<Decimal>,
    /// NAV change since prior close, USD.
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    /// NAV change since prior close as a fraction.
    #[serde(rename = "netPercentChange", default, with = "decimal_opt")]
    pub net_percent_change: Option<Decimal>,
    /// Security status string.
    #[serde(rename = "securityStatus", default)]
    pub security_status: Option<String>,
    /// Cumulative session volume (shares).
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Last trade time in epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

/// Static reference data for a mutual-fund quote.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReferenceMutualFund {
    /// CUSIP.
    #[serde(default)]
    pub cusip: Option<String>,
    /// Fund description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab exchange code.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Exchange display name.
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
}

// --- Enums ---

string_enum! {
    /// Asset class discriminator on a quote response.
    AssetMainType {
        /// Bond. Schwab returns no typed schema for bond quotes.
        Bond = "BOND",
        /// Equity.
        Equity = "EQUITY",
        /// Forex pair.
        Forex = "FOREX",
        /// Futures contract.
        Future = "FUTURE",
        /// Futures option.
        FutureOption = "FUTURE_OPTION",
        /// Index.
        Index = "INDEX",
        /// Mutual fund.
        MutualFund = "MUTUAL_FUND",
        /// Listed option.
        Option = "OPTION",
    }
}

string_enum! {
    /// Asset sub-type (only applicable to some asset classes).
    AssetSubType {
        /// Common stock.
        Coe = "COE",
        /// Preferred stock.
        Prf = "PRF",
        /// American Depositary Receipt.
        Adr = "ADR",
        /// Global Depositary Receipt.
        Gdr = "GDR",
        /// Closed-end fund.
        Cef = "CEF",
        /// Exchange-traded fund.
        Etf = "ETF",
        /// Exchange-traded note.
        Etn = "ETN",
        /// Unit investment trust.
        Uit = "UIT",
        /// Warrant.
        War = "WAR",
        /// Right.
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
        /// Actively managed.
        Active = "A",
        /// Leveraged.
        Leveraged = "L",
        /// Passive/index-tracking.
        Passive = "P",
        /// Quantitative/rules-based.
        Quantitative = "Q",
        /// Inverse/short.
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
        /// Put.
        Put = "P",
        /// Call.
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
        /// `quote` sub-object (live bid/ask/last).
        Quote = "quote",
        /// `fundamental` sub-object (dividends, P/E, volumes).
        Fundamental = "fundamental",
        /// `extended` sub-object (pre-/post-market data).
        Extended = "extended",
        /// `reference` sub-object (CUSIP, exchange, classification).
        Reference = "reference",
        /// `regular` sub-object (last regular-session trade).
        Regular = "regular",
        /// All sub-objects (default).
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
