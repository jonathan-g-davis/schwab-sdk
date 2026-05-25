//! `/instruments` and `/instruments/{cusip_id}` - instrument lookup.
//!
//! - `search` does a symbol/description search, optionally returning
//!   fundamental data when [`Projection::Fundamental`] is used.
//! - `get_by_cusip` fetches basic instrument details for a single CUSIP.
//!
//! Reached through
//! [`MarketData::instruments`](super::MarketData::instruments).

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;

/// Accessor for `/instruments*`. Construct via
/// [`MarketData::instruments`](super::MarketData::instruments).
pub struct Instruments<'a> {
    client: &'a SchwabClient,
}

impl<'a> Instruments<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// `GET /instruments?symbol=...&projection=...` - search for
    /// instruments. `symbol` is interpreted per the `projection`: an
    /// exact/regex symbol match, a description search, or a
    /// fundamental-data lookup. See [`Projection`].
    pub async fn search(
        &self,
        symbol: impl AsRef<str>,
        projection: Projection,
    ) -> Result<InstrumentsResponse> {
        let projection = projection.to_string();
        self.client
            .market_data_http()
            .get("/instruments")
            .query(&[
                ("symbol", symbol.as_ref()),
                ("projection", projection.as_str()),
            ])
            .send_json()
            .await
    }

    /// `GET /instruments/{cusip_id}` - fetch basic instrument details by
    /// CUSIP. Per Schwab's OpenAPI spec this returns a bare
    /// [`InstrumentResponse`] (not the `{instruments: [...]}` wrapper the
    /// search endpoint uses).
    pub async fn get_by_cusip(&self, cusip: impl AsRef<str>) -> Result<InstrumentResponse> {
        let path = format!("/instruments/{}", cusip.as_ref());
        self.client.market_data_http().get_json(&path).await
    }
}

// --- Response shape ---

/// `GET /instruments` (search) response body.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct InstrumentsResponse {
    /// One entry per matched instrument; empty when nothing matched.
    #[serde(default)]
    pub instruments: Vec<InstrumentResponse>,
}

/// One instrument record. Search results without
/// [`Projection::Fundamental`] populate only the identity fields;
/// `fundamental` is present only for fundamental-projection searches.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct InstrumentResponse {
    /// CUSIP.
    #[serde(default)]
    pub cusip: Option<String>,
    /// Wire symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Issuer/instrument description.
    #[serde(default)]
    pub description: Option<String>,
    /// Exchange display name (e.g. `"NASDAQ"`).
    #[serde(default)]
    pub exchange: Option<String>,
    /// Asset class.
    #[serde(rename = "assetType", default)]
    pub asset_type: Option<InstrumentAssetType>,
    /// Bond factor, as Schwab ships it (a string on the wire).
    #[serde(rename = "bondFactor", default)]
    pub bond_factor: Option<String>,
    /// Bond multiplier, as Schwab ships it (a string on the wire).
    #[serde(rename = "bondMultiplier", default)]
    pub bond_multiplier: Option<String>,
    /// Last bond price, USD.
    #[serde(rename = "bondPrice", default, with = "decimal_opt")]
    pub bond_price: Option<Decimal>,
    /// Present only for [`Projection::Fundamental`] searches.
    #[serde(default)]
    pub fundamental: Option<FundamentalInst>,
    /// Nested basic-identity block (some responses include it alongside
    /// the top-level identity fields).
    #[serde(rename = "instrumentInfo", default)]
    pub instrument_info: Option<Instrument>,
    /// Bond-specific detail block; `None` for non-bond asset types.
    #[serde(rename = "bondInstrumentInfo", default)]
    pub bond_instrument_info: Option<Bond>,
}

/// Basic instrument identity block.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Instrument {
    /// CUSIP.
    #[serde(default)]
    pub cusip: Option<String>,
    /// Wire symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Exchange display name.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Asset class.
    #[serde(rename = "assetType", default)]
    pub asset_type: Option<InstrumentAssetType>,
}

/// Bond-specific instrument block.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Bond {
    /// CUSIP.
    #[serde(default)]
    pub cusip: Option<String>,
    /// Wire symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Bond description.
    #[serde(default)]
    pub description: Option<String>,
    /// Exchange display name.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Asset class (always [`InstrumentAssetType::Bond`]).
    #[serde(rename = "assetType", default)]
    pub asset_type: Option<InstrumentAssetType>,
    /// Bond factor as a wire string (remaining principal fraction).
    #[serde(rename = "bondFactor", default)]
    pub bond_factor: Option<String>,
    /// Bond multiplier as a wire string (units per contract).
    #[serde(rename = "bondMultiplier", default)]
    pub bond_multiplier: Option<String>,
    /// Last bond price, USD.
    #[serde(rename = "bondPrice", default, with = "decimal_opt")]
    pub bond_price: Option<Decimal>,
}

/// Fundamental data block, returned for [`Projection::Fundamental`]
/// searches. Every field is optional; Schwab populates the subset it
/// has for the instrument. Date-like fields are kept as `String` (Schwab
/// ships them in a variety of formats here).
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct FundamentalInst {
    /// Wire symbol the fundamentals belong to.
    #[serde(default)]
    pub symbol: Option<String>,
    /// 52-week high price, USD.
    #[serde(default, with = "decimal_opt")]
    pub high52: Option<Decimal>,
    /// 52-week low price, USD.
    #[serde(default, with = "decimal_opt")]
    pub low52: Option<Decimal>,
    /// Most recent dividend amount, USD per share.
    #[serde(rename = "dividendAmount", default, with = "decimal_opt")]
    pub dividend_amount: Option<Decimal>,
    /// Trailing dividend yield as a fraction.
    #[serde(rename = "dividendYield", default, with = "decimal_opt")]
    pub dividend_yield: Option<Decimal>,
    /// Most recent dividend date (Schwab-formatted string).
    #[serde(rename = "dividendDate", default)]
    pub dividend_date: Option<String>,
    /// Price-to-earnings ratio.
    #[serde(rename = "peRatio", default, with = "decimal_opt")]
    pub pe_ratio: Option<Decimal>,
    /// Price/earnings-to-growth ratio.
    #[serde(rename = "pegRatio", default, with = "decimal_opt")]
    pub peg_ratio: Option<Decimal>,
    /// Price-to-book ratio.
    #[serde(rename = "pbRatio", default, with = "decimal_opt")]
    pub pb_ratio: Option<Decimal>,
    /// Price-to-revenue ratio.
    #[serde(rename = "prRatio", default, with = "decimal_opt")]
    pub pr_ratio: Option<Decimal>,
    /// Price-to-cash-flow ratio.
    #[serde(rename = "pcfRatio", default, with = "decimal_opt")]
    pub pcf_ratio: Option<Decimal>,
    /// Trailing-twelve-month gross margin (fraction).
    #[serde(rename = "grossMarginTTM", default, with = "decimal_opt")]
    pub gross_margin_ttm: Option<Decimal>,
    /// Most-recent-quarter gross margin (fraction).
    #[serde(rename = "grossMarginMRQ", default, with = "decimal_opt")]
    pub gross_margin_mrq: Option<Decimal>,
    /// Trailing-twelve-month net profit margin (fraction).
    #[serde(rename = "netProfitMarginTTM", default, with = "decimal_opt")]
    pub net_profit_margin_ttm: Option<Decimal>,
    /// Most-recent-quarter net profit margin (fraction).
    #[serde(rename = "netProfitMarginMRQ", default, with = "decimal_opt")]
    pub net_profit_margin_mrq: Option<Decimal>,
    /// Trailing-twelve-month operating margin (fraction).
    #[serde(rename = "operatingMarginTTM", default, with = "decimal_opt")]
    pub operating_margin_ttm: Option<Decimal>,
    /// Most-recent-quarter operating margin (fraction).
    #[serde(rename = "operatingMarginMRQ", default, with = "decimal_opt")]
    pub operating_margin_mrq: Option<Decimal>,
    /// Return on equity (fraction).
    #[serde(rename = "returnOnEquity", default, with = "decimal_opt")]
    pub return_on_equity: Option<Decimal>,
    /// Return on assets (fraction).
    #[serde(rename = "returnOnAssets", default, with = "decimal_opt")]
    pub return_on_assets: Option<Decimal>,
    /// Return on invested capital (fraction).
    #[serde(rename = "returnOnInvestment", default, with = "decimal_opt")]
    pub return_on_investment: Option<Decimal>,
    /// Quick (acid-test) ratio.
    #[serde(rename = "quickRatio", default, with = "decimal_opt")]
    pub quick_ratio: Option<Decimal>,
    /// Current ratio.
    #[serde(rename = "currentRatio", default, with = "decimal_opt")]
    pub current_ratio: Option<Decimal>,
    /// Interest coverage ratio.
    #[serde(rename = "interestCoverage", default, with = "decimal_opt")]
    pub interest_coverage: Option<Decimal>,
    /// Total debt to total capital (fraction).
    #[serde(rename = "totalDebtToCapital", default, with = "decimal_opt")]
    pub total_debt_to_capital: Option<Decimal>,
    /// Long-term debt to equity (fraction).
    #[serde(rename = "ltDebtToEquity", default, with = "decimal_opt")]
    pub lt_debt_to_equity: Option<Decimal>,
    /// Total debt to equity (fraction).
    #[serde(rename = "totalDebtToEquity", default, with = "decimal_opt")]
    pub total_debt_to_equity: Option<Decimal>,
    /// Trailing-twelve-month EPS, USD.
    #[serde(rename = "epsTTM", default, with = "decimal_opt")]
    pub eps_ttm: Option<Decimal>,
    /// Year-over-year change in TTM EPS (fraction).
    #[serde(rename = "epsChangePercentTTM", default, with = "decimal_opt")]
    pub eps_change_percent_ttm: Option<Decimal>,
    /// Year-over-year change in annual EPS, USD.
    #[serde(rename = "epsChangeYear", default, with = "decimal_opt")]
    pub eps_change_year: Option<Decimal>,
    /// EPS change since the prior period, USD.
    #[serde(rename = "epsChange", default, with = "decimal_opt")]
    pub eps_change: Option<Decimal>,
    /// Year-over-year revenue change (fraction).
    #[serde(rename = "revChangeYear", default, with = "decimal_opt")]
    pub rev_change_year: Option<Decimal>,
    /// TTM revenue change (fraction).
    #[serde(rename = "revChangeTTM", default, with = "decimal_opt")]
    pub rev_change_ttm: Option<Decimal>,
    /// In-period revenue change (fraction).
    #[serde(rename = "revChangeIn", default, with = "decimal_opt")]
    pub rev_change_in: Option<Decimal>,
    /// Total shares outstanding.
    #[serde(rename = "sharesOutstanding", default, with = "decimal_opt")]
    pub shares_outstanding: Option<Decimal>,
    /// Float market cap (public-float shares × price), USD.
    #[serde(rename = "marketCapFloat", default, with = "decimal_opt")]
    pub market_cap_float: Option<Decimal>,
    /// Total market cap (all shares × price), USD.
    #[serde(rename = "marketCap", default, with = "decimal_opt")]
    pub market_cap: Option<Decimal>,
    /// Book value per share, USD.
    #[serde(rename = "bookValuePerShare", default, with = "decimal_opt")]
    pub book_value_per_share: Option<Decimal>,
    /// Short interest as a fraction of float.
    #[serde(rename = "shortIntToFloat", default, with = "decimal_opt")]
    pub short_int_to_float: Option<Decimal>,
    /// Days-to-cover (short interest/average daily volume).
    #[serde(rename = "shortIntDayToCover", default, with = "decimal_opt")]
    pub short_int_day_to_cover: Option<Decimal>,
    /// 3-year dividend growth rate (fraction).
    #[serde(rename = "divGrowthRate3Year", default, with = "decimal_opt")]
    pub div_growth_rate_3_year: Option<Decimal>,
    /// Most recent dividend pay amount, USD per share.
    #[serde(rename = "dividendPayAmount", default, with = "decimal_opt")]
    pub dividend_pay_amount: Option<Decimal>,
    /// Most recent dividend pay date (Schwab-formatted string).
    #[serde(rename = "dividendPayDate", default)]
    pub dividend_pay_date: Option<String>,
    /// Beta relative to the broader market.
    #[serde(default, with = "decimal_opt")]
    pub beta: Option<Decimal>,
    /// 1-day average volume (shares).
    #[serde(rename = "vol1DayAvg", default, with = "decimal_opt")]
    pub vol_1_day_avg: Option<Decimal>,
    /// 10-day average volume (shares).
    #[serde(rename = "vol10DayAvg", default, with = "decimal_opt")]
    pub vol_10_day_avg: Option<Decimal>,
    /// 3-month average volume (shares).
    #[serde(rename = "vol3MonthAvg", default, with = "decimal_opt")]
    pub vol_3_month_avg: Option<Decimal>,
    /// 10-day average volume (integer-typed alias).
    #[serde(rename = "avg10DaysVolume", default)]
    pub avg_10_days_volume: Option<i64>,
    /// 1-day average volume (integer-typed alias).
    #[serde(rename = "avg1DayVolume", default)]
    pub avg_1_day_volume: Option<i64>,
    /// 3-month average volume (integer-typed alias).
    #[serde(rename = "avg3MonthVolume", default)]
    pub avg_3_month_volume: Option<i64>,
    /// Dividend declaration date (Schwab-formatted string).
    #[serde(rename = "declarationDate", default)]
    pub declaration_date: Option<String>,
    /// Number of dividends per year (4 = quarterly, etc.).
    #[serde(rename = "dividendFreq", default)]
    pub dividend_freq: Option<i32>,
    /// Reported EPS, USD.
    #[serde(default, with = "decimal_opt")]
    pub eps: Option<Decimal>,
    /// Date of the most recent corporate action (Schwab-formatted string).
    #[serde(rename = "corpactionDate", default)]
    pub corpaction_date: Option<String>,
    /// Day-trade-notional volume.
    #[serde(rename = "dtnVolume", default)]
    pub dtn_volume: Option<i64>,
    /// Next projected dividend pay date (Schwab-formatted string).
    #[serde(rename = "nextDividendPayDate", default)]
    pub next_dividend_pay_date: Option<String>,
    /// Next projected dividend ex-date (Schwab-formatted string).
    #[serde(rename = "nextDividendDate", default)]
    pub next_dividend_date: Option<String>,
    /// Leverage factor for leveraged funds.
    #[serde(rename = "fundLeverageFactor", default, with = "decimal_opt")]
    pub fund_leverage_factor: Option<Decimal>,
    /// Fund strategy code (kept as a string here; see
    /// [`crate::market_data::FundStrategy`] for the typed equivalent).
    #[serde(rename = "fundStrategy", default)]
    pub fund_strategy: Option<String>,
}

// --- Enums ---

string_enum! {
    /// `projection` query value: how `symbol` is interpreted by the
    /// search endpoint.
    Projection {
        /// Exact symbol match.
        SymbolSearch = "symbol-search",
        /// Symbol regex match.
        SymbolRegex = "symbol-regex",
        /// Description text search.
        DescSearch = "desc-search",
        /// Description regex match.
        DescRegex = "desc-regex",
        /// General search.
        Search = "search",
        /// Return fundamental data for the matched instrument(s).
        Fundamental = "fundamental",
    }
}

string_enum! {
    /// `assetType` discriminator on an instrument record.
    InstrumentAssetType {
        /// Bond.
        Bond = "BOND",
        /// Listed equity.
        Equity = "EQUITY",
        /// Exchange-traded fund.
        Etf = "ETF",
        /// Schwab extended-hours classification.
        Extended = "EXTENDED",
        /// Forex pair.
        Forex = "FOREX",
        /// Futures contract.
        Future = "FUTURE",
        /// Futures option.
        FutureOption = "FUTURE_OPTION",
        /// Fundamental-data record.
        Fundamental = "FUNDAMENTAL",
        /// Index.
        Index = "INDEX",
        /// Technical indicator.
        Indicator = "INDICATOR",
        /// Mutual fund.
        MutualFund = "MUTUAL_FUND",
        /// Listed option.
        Option_ = "OPTION",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn instruments_search_response_parses() {
        let json = r#"{
            "instruments": [
                {
                    "cusip": "037833100",
                    "symbol": "AAPL",
                    "description": "Apple Inc",
                    "exchange": "NASDAQ",
                    "assetType": "EQUITY"
                }
            ]
        }"#;
        let resp: InstrumentsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.instruments.len(), 1);
        let inst = &resp.instruments[0];
        assert_eq!(inst.symbol.as_deref(), Some("AAPL"));
        assert_eq!(inst.cusip.as_deref(), Some("037833100"));
        assert_eq!(inst.asset_type, Some(InstrumentAssetType::Equity));
        assert!(inst.fundamental.is_none());
    }

    #[test]
    fn fundamental_projection_response_parses() {
        let json = r#"{
            "instruments": [
                {
                    "cusip": "037833100",
                    "symbol": "AAPL",
                    "description": "Apple Inc",
                    "exchange": "NASDAQ",
                    "assetType": "EQUITY",
                    "fundamental": {
                        "symbol": "AAPL",
                        "high52": 199.62,
                        "low52": 164.08,
                        "peRatio": 28.599,
                        "marketCap": 2700000000000.0,
                        "eps": 6.13,
                        "dividendAmount": 0.96,
                        "dividendFreq": 4,
                        "avg10DaysVolume": 52000000,
                        "beta": 1.29,
                        "fundStrategy": "A"
                    }
                }
            ]
        }"#;
        let resp: InstrumentsResponse = serde_json::from_str(json).unwrap();
        let f = resp.instruments[0].fundamental.as_ref().unwrap();
        assert_eq!(f.symbol.as_deref(), Some("AAPL"));
        assert_eq!(f.high52, Some(dec!(199.62)));
        assert_eq!(f.low52, Some(dec!(164.08)));
        assert_eq!(f.pe_ratio, Some(dec!(28.599)));
        assert_eq!(f.eps, Some(dec!(6.13)));
        assert_eq!(f.dividend_amount, Some(dec!(0.96)));
        assert_eq!(f.dividend_freq, Some(4));
        assert_eq!(f.avg_10_days_volume, Some(52000000));
        assert_eq!(f.beta, Some(dec!(1.29)));
        assert_eq!(f.fund_strategy.as_deref(), Some("A"));
    }

    #[test]
    fn by_cusip_response_parses_as_bare_instrument() {
        // Per the OpenAPI spec the by-cusip endpoint returns a bare
        // InstrumentResponse, not the {instruments:[...]} wrapper.
        let json = r#"{
            "cusip": "037833100",
            "symbol": "AAPL",
            "description": "Apple Inc",
            "exchange": "NASDAQ",
            "assetType": "EQUITY"
        }"#;
        let inst: InstrumentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(inst.symbol.as_deref(), Some("AAPL"));
        assert_eq!(inst.asset_type, Some(InstrumentAssetType::Equity));
    }

    #[test]
    fn bond_instrument_response_parses() {
        let json = r#"{
            "cusip": "912828YK0",
            "symbol": "912828YK0",
            "description": "US TREASURY NOTE",
            "assetType": "BOND",
            "bondFactor": "1.00000000",
            "bondMultiplier": "1000",
            "bondPrice": 99.5,
            "bondInstrumentInfo": {
                "cusip": "912828YK0",
                "assetType": "BOND",
                "bondPrice": 99.5
            }
        }"#;
        let inst: InstrumentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(inst.asset_type, Some(InstrumentAssetType::Bond));
        assert_eq!(inst.bond_factor.as_deref(), Some("1.00000000"));
        assert_eq!(inst.bond_multiplier.as_deref(), Some("1000"));
        assert_eq!(inst.bond_price, Some(dec!(99.5)));
        let bond = inst.bond_instrument_info.as_ref().unwrap();
        assert_eq!(bond.bond_price, Some(dec!(99.5)));
    }

    #[test]
    fn empty_instruments_response_parses() {
        let resp: InstrumentsResponse = serde_json::from_str(r#"{"instruments": []}"#).unwrap();
        assert!(resp.instruments.is_empty());
        let resp: InstrumentsResponse = serde_json::from_str("{}").unwrap();
        assert!(resp.instruments.is_empty());
    }

    #[test]
    fn projection_round_trips_known_variants() {
        for raw in [
            "symbol-search",
            "symbol-regex",
            "desc-search",
            "desc-regex",
            "search",
            "fundamental",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: Projection = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn unknown_instrument_asset_type_preserves_raw_string() {
        let parsed: InstrumentAssetType = serde_json::from_str(r#""CRYPTO""#).unwrap();
        assert!(matches!(parsed, InstrumentAssetType::Unknown(ref s) if s == "CRYPTO"));
    }
}
