//! `/chains` - option chain for an optionable symbol.
//!
//! Returns the option contracts for a symbol, grouped by expiration and
//! then by strike. The contract grouping is two levels of map:
//!
//! - outer key is `"<expiration-date>:<days-to-expiration>"`, e.g.
//!   `"2024-01-19:5"`;
//! - inner key is the strike price as the string Schwab sends it, e.g.
//!   `"150.0"`;
//! - the value is the [`OptionContract`]s at that strike.
//!
//! The per-strike value is exposed as a `Vec<OptionContract>`. Schwab's
//! published schema types it as a single contract object; deserialization
//! also accepts an array at that position, normalizing either shape to the
//! list form.
//!
//! Reached through [`MarketData::chains`](super::MarketData::chains).

use std::collections::HashMap;
use std::fmt;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;

/// Accessor for `/chains`. Construct via
/// [`MarketData::chains`](super::MarketData::chains).
pub struct Chains<'a> {
    client: &'a SchwabClient,
}

impl<'a> Chains<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Begin a `GET /chains` request for an optionable `symbol`. Every
    /// filter is optional; with none set Schwab returns the full
    /// `SINGLE`-strategy chain.
    pub fn get(&self, symbol: impl Into<String>) -> GetChainBuilder<'a> {
        GetChainBuilder {
            client: self.client,
            symbol: symbol.into(),
            contract_type: None,
            strike_count: None,
            include_underlying_quote: None,
            strategy: None,
            interval: None,
            strike: None,
            range: None,
            from_date: None,
            to_date: None,
            volatility: None,
            underlying_price: None,
            interest_rate: None,
            days_to_expiration: None,
            exp_month: None,
            option_type: None,
            entitlement: None,
        }
    }
}

/// In-flight request for `GET /chains`. Built via [`Chains::get`].
#[must_use = "call .send() to execute the request"]
pub struct GetChainBuilder<'a> {
    client: &'a SchwabClient,
    symbol: String,
    contract_type: Option<ContractType>,
    strike_count: Option<i32>,
    include_underlying_quote: Option<bool>,
    strategy: Option<OptionStrategy>,
    interval: Option<Decimal>,
    strike: Option<Decimal>,
    range: Option<OptionRange>,
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
    volatility: Option<Decimal>,
    underlying_price: Option<Decimal>,
    interest_rate: Option<Decimal>,
    days_to_expiration: Option<i32>,
    exp_month: Option<ExpirationMonth>,
    option_type: Option<OptionType>,
    entitlement: Option<Entitlement>,
}

impl<'a> GetChainBuilder<'a> {
    /// Restrict the chain to calls, puts, or both.
    pub fn contract_type(mut self, value: ContractType) -> Self {
        self.contract_type = Some(value);
        self
    }

    /// Number of strikes to return above and below the at-the-money
    /// price.
    pub fn strike_count(mut self, value: i32) -> Self {
        self.strike_count = Some(value);
        self
    }

    /// Include the underlying's quote in [`OptionChain::underlying`].
    pub fn include_underlying_quote(mut self, value: bool) -> Self {
        self.include_underlying_quote = Some(value);
        self
    }

    /// Chain strategy. `ANALYTICAL` enables the theoretical-value
    /// parameters ([`Self::volatility`], [`Self::underlying_price`],
    /// [`Self::interest_rate`], [`Self::days_to_expiration`]).
    pub fn strategy(mut self, value: OptionStrategy) -> Self {
        self.strategy = Some(value);
        self
    }

    /// Strike interval for spread-strategy chains.
    pub fn interval(mut self, value: Decimal) -> Self {
        self.interval = Some(value);
        self
    }

    /// Restrict the chain to a single strike price.
    pub fn strike(mut self, value: Decimal) -> Self {
        self.strike = Some(value);
        self
    }

    /// Restrict the chain to a moneyness range (ITM/NTM/OTM etc.).
    pub fn range(mut self, value: OptionRange) -> Self {
        self.range = Some(value);
        self
    }

    /// Lower bound of the expiration window (`yyyy-MM-dd`).
    pub fn from_date(mut self, value: NaiveDate) -> Self {
        self.from_date = Some(value);
        self
    }

    /// Upper bound of the expiration window (`yyyy-MM-dd`).
    pub fn to_date(mut self, value: NaiveDate) -> Self {
        self.to_date = Some(value);
        self
    }

    /// Volatility for theoretical-value math. Applies only to the
    /// `ANALYTICAL` strategy.
    pub fn volatility(mut self, value: Decimal) -> Self {
        self.volatility = Some(value);
        self
    }

    /// Underlying price for theoretical-value math. Applies only to the
    /// `ANALYTICAL` strategy.
    pub fn underlying_price(mut self, value: Decimal) -> Self {
        self.underlying_price = Some(value);
        self
    }

    /// Interest rate for theoretical-value math. Applies only to the
    /// `ANALYTICAL` strategy.
    pub fn interest_rate(mut self, value: Decimal) -> Self {
        self.interest_rate = Some(value);
        self
    }

    /// Days to expiration for theoretical-value math. Applies only to
    /// the `ANALYTICAL` strategy.
    pub fn days_to_expiration(mut self, value: i32) -> Self {
        self.days_to_expiration = Some(value);
        self
    }

    /// Restrict the chain to a single expiration month.
    pub fn exp_month(mut self, value: ExpirationMonth) -> Self {
        self.exp_month = Some(value);
        self
    }

    /// Restrict the chain to standard or non-standard contracts.
    pub fn option_type(mut self, value: OptionType) -> Self {
        self.option_type = Some(value);
        self
    }

    /// Client entitlement; applies only when authenticated with a retail
    /// token.
    pub fn entitlement(mut self, value: Entitlement) -> Self {
        self.entitlement = Some(value);
        self
    }

    pub async fn send(self) -> Result<OptionChain> {
        let md = self.client.market_data_http();
        let mut request = md.get("/chains").query(&[("symbol", self.symbol.as_str())]);
        if let Some(v) = &self.contract_type {
            let s = v.to_string();
            request = request.query(&[("contractType", s.as_str())]);
        }
        if let Some(v) = self.strike_count {
            let s = v.to_string();
            request = request.query(&[("strikeCount", s.as_str())]);
        }
        if let Some(v) = self.include_underlying_quote {
            let s = if v { "true" } else { "false" };
            request = request.query(&[("includeUnderlyingQuote", s)]);
        }
        if let Some(v) = &self.strategy {
            let s = v.to_string();
            request = request.query(&[("strategy", s.as_str())]);
        }
        if let Some(v) = self.interval {
            let s = v.to_string();
            request = request.query(&[("interval", s.as_str())]);
        }
        if let Some(v) = self.strike {
            let s = v.to_string();
            request = request.query(&[("strike", s.as_str())]);
        }
        if let Some(v) = &self.range {
            let s = v.to_string();
            request = request.query(&[("range", s.as_str())]);
        }
        if let Some(v) = self.from_date {
            let s = v.format("%Y-%m-%d").to_string();
            request = request.query(&[("fromDate", s.as_str())]);
        }
        if let Some(v) = self.to_date {
            let s = v.format("%Y-%m-%d").to_string();
            request = request.query(&[("toDate", s.as_str())]);
        }
        if let Some(v) = self.volatility {
            let s = v.to_string();
            request = request.query(&[("volatility", s.as_str())]);
        }
        if let Some(v) = self.underlying_price {
            let s = v.to_string();
            request = request.query(&[("underlyingPrice", s.as_str())]);
        }
        if let Some(v) = self.interest_rate {
            let s = v.to_string();
            request = request.query(&[("interestRate", s.as_str())]);
        }
        if let Some(v) = self.days_to_expiration {
            let s = v.to_string();
            request = request.query(&[("daysToExpiration", s.as_str())]);
        }
        if let Some(v) = &self.exp_month {
            let s = v.to_string();
            request = request.query(&[("expMonth", s.as_str())]);
        }
        if let Some(v) = &self.option_type {
            let s = v.to_string();
            request = request.query(&[("optionType", s.as_str())]);
        }
        if let Some(v) = &self.entitlement {
            let s = v.to_string();
            request = request.query(&[("entitlement", s.as_str())]);
        }
        md.execute_json(request).await
    }
}

// --- Response shape ---

/// Per-strike option contracts for one expiration. Keyed by strike price
/// as the string Schwab sends (e.g. `"150.0"`); the value is the list of
/// contracts at that strike.
pub type OptionContractMap = HashMap<String, Vec<OptionContract>>;

/// Deserialize a `callExpDateMap` / `putExpDateMap`.
///
/// Schwab's published schema types the per-strike value as a single
/// [`OptionContract`]; an array of contracts can also appear at that
/// position. This accepts either shape and normalizes both to a `Vec`.
fn de_exp_date_map<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, OptionContractMap>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: HashMap<String, HashMap<String, Contracts>> = HashMap::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|(expiration, strikes)| {
            let strikes = strikes
                .into_iter()
                .map(|(strike, contracts)| (strike, contracts.0))
                .collect();
            (expiration, strikes)
        })
        .collect())
}

/// One strike's contracts, tolerant of both the single-object and array
/// wire shapes. Private: callers see the normalized `Vec<OptionContract>`.
struct Contracts(Vec<OptionContract>);

impl<'de> Deserialize<'de> for Contracts {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer
            .deserialize_any(ContractsVisitor)
            .map(Contracts)
    }
}

struct ContractsVisitor;

impl<'de> Visitor<'de> for ContractsVisitor {
    type Value = Vec<OptionContract>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an option contract or an array of option contracts")
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut contracts = Vec::new();
        while let Some(contract) = seq.next_element()? {
            contracts.push(contract);
        }
        Ok(contracts)
    }

    fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let contract = OptionContract::deserialize(MapAccessDeserializer::new(map))?;
        Ok(vec![contract])
    }
}

/// `/chains` response body.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OptionChain {
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    /// Underlying quote; populated when `include_underlying_quote` was
    /// set on the request.
    #[serde(default)]
    pub underlying: Option<Underlying>,
    #[serde(default)]
    pub strategy: Option<OptionStrategy>,
    #[serde(default, with = "decimal_opt")]
    pub interval: Option<Decimal>,
    #[serde(rename = "isDelayed", default)]
    pub is_delayed: Option<bool>,
    #[serde(rename = "isIndex", default)]
    pub is_index: Option<bool>,
    #[serde(rename = "daysToExpiration", default, with = "decimal_opt")]
    pub days_to_expiration: Option<Decimal>,
    #[serde(rename = "interestRate", default, with = "decimal_opt")]
    pub interest_rate: Option<Decimal>,
    #[serde(rename = "underlyingPrice", default, with = "decimal_opt")]
    pub underlying_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub volatility: Option<Decimal>,
    /// Call contracts, keyed by `"<expiration>:<days-to-expiration>"`.
    #[serde(
        rename = "callExpDateMap",
        default,
        deserialize_with = "de_exp_date_map"
    )]
    pub call_exp_date_map: HashMap<String, OptionContractMap>,
    /// Put contracts, keyed by `"<expiration>:<days-to-expiration>"`.
    #[serde(
        rename = "putExpDateMap",
        default,
        deserialize_with = "de_exp_date_map"
    )]
    pub put_exp_date_map: HashMap<String, OptionContractMap>,
}

/// Underlying-security snapshot attached to an [`OptionChain`].
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Underlying {
    #[serde(default, with = "decimal_opt")]
    pub ask: Option<Decimal>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub bid: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub change: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub close: Option<Decimal>,
    #[serde(default)]
    pub delayed: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<UnderlyingExchange>,
    #[serde(rename = "fiftyTwoWeekHigh", default, with = "decimal_opt")]
    pub fifty_two_week_high: Option<Decimal>,
    #[serde(rename = "fiftyTwoWeekLow", default, with = "decimal_opt")]
    pub fifty_two_week_low: Option<Decimal>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub last: Option<Decimal>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub mark: Option<Decimal>,
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    #[serde(rename = "markPercentChange", default, with = "decimal_opt")]
    pub mark_percent_change: Option<Decimal>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    #[serde(rename = "percentChange", default, with = "decimal_opt")]
    pub percent_change: Option<Decimal>,
    /// Epoch milliseconds.
    #[serde(rename = "quoteTime", default)]
    pub quote_time: Option<i64>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Epoch milliseconds.
    #[serde(rename = "tradeTime", default)]
    pub trade_time: Option<i64>,
}

/// A single option contract within a [`OptionChain`].
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OptionContract {
    #[serde(rename = "putCall", default)]
    pub put_call: Option<PutCall>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "exchangeName", default)]
    pub exchange_name: Option<String>,
    #[serde(rename = "bidPrice", default, with = "decimal_opt")]
    pub bid_price: Option<Decimal>,
    #[serde(rename = "askPrice", default, with = "decimal_opt")]
    pub ask_price: Option<Decimal>,
    #[serde(rename = "lastPrice", default, with = "decimal_opt")]
    pub last_price: Option<Decimal>,
    #[serde(rename = "markPrice", default, with = "decimal_opt")]
    pub mark_price: Option<Decimal>,
    #[serde(rename = "bidSize", default)]
    pub bid_size: Option<i64>,
    #[serde(rename = "askSize", default)]
    pub ask_size: Option<i64>,
    #[serde(rename = "lastSize", default)]
    pub last_size: Option<i64>,
    #[serde(rename = "highPrice", default, with = "decimal_opt")]
    pub high_price: Option<Decimal>,
    #[serde(rename = "lowPrice", default, with = "decimal_opt")]
    pub low_price: Option<Decimal>,
    #[serde(rename = "openPrice", default, with = "decimal_opt")]
    pub open_price: Option<Decimal>,
    #[serde(rename = "closePrice", default, with = "decimal_opt")]
    pub close_price: Option<Decimal>,
    #[serde(rename = "totalVolume", default)]
    pub total_volume: Option<i64>,
    /// Epoch milliseconds.
    #[serde(rename = "tradeDate", default)]
    pub trade_date: Option<i64>,
    /// Epoch milliseconds. Schwab's published schema mistypes this as a
    /// 32-bit integer; the live API sends a millisecond timestamp.
    #[serde(rename = "quoteTimeInLong", default)]
    pub quote_time_in_long: Option<i64>,
    /// Epoch milliseconds. Same schema mistype as
    /// [`Self::quote_time_in_long`].
    #[serde(rename = "tradeTimeInLong", default)]
    pub trade_time_in_long: Option<i64>,
    #[serde(rename = "netChange", default, with = "decimal_opt")]
    pub net_change: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub volatility: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub delta: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub gamma: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub theta: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub vega: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub rho: Option<Decimal>,
    #[serde(rename = "timeValue", default, with = "decimal_opt")]
    pub time_value: Option<Decimal>,
    #[serde(rename = "openInterest", default, with = "decimal_opt")]
    pub open_interest: Option<Decimal>,
    #[serde(rename = "isInTheMoney", default)]
    pub is_in_the_money: Option<bool>,
    #[serde(rename = "theoreticalOptionValue", default, with = "decimal_opt")]
    pub theoretical_option_value: Option<Decimal>,
    #[serde(rename = "theoreticalVolatility", default, with = "decimal_opt")]
    pub theoretical_volatility: Option<Decimal>,
    #[serde(rename = "isMini", default)]
    pub is_mini: Option<bool>,
    #[serde(rename = "isNonStandard", default)]
    pub is_non_standard: Option<bool>,
    #[serde(rename = "optionDeliverablesList", default)]
    pub option_deliverables_list: Vec<OptionDeliverables>,
    #[serde(rename = "strikePrice", default, with = "decimal_opt")]
    pub strike_price: Option<Decimal>,
    /// `yyyy-MM-dd'T'HH:mm:ss` expiration timestamp string.
    #[serde(rename = "expirationDate", default)]
    pub expiration_date: Option<String>,
    #[serde(rename = "daysToExpiration", default)]
    pub days_to_expiration: Option<i32>,
    #[serde(rename = "expirationType", default)]
    pub expiration_type: Option<ExpirationType>,
    /// Epoch milliseconds.
    #[serde(rename = "lastTradingDay", default)]
    pub last_trading_day: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    #[serde(rename = "settlementType", default)]
    pub settlement_type: Option<SettlementType>,
    #[serde(rename = "deliverableNote", default)]
    pub deliverable_note: Option<String>,
    #[serde(rename = "isIndexOption", default)]
    pub is_index_option: Option<bool>,
    #[serde(rename = "percentChange", default, with = "decimal_opt")]
    pub percent_change: Option<Decimal>,
    #[serde(rename = "markChange", default, with = "decimal_opt")]
    pub mark_change: Option<Decimal>,
    #[serde(rename = "markPercentChange", default, with = "decimal_opt")]
    pub mark_percent_change: Option<Decimal>,
    #[serde(rename = "isPennyPilot", default)]
    pub is_penny_pilot: Option<bool>,
    #[serde(rename = "intrinsicValue", default, with = "decimal_opt")]
    pub intrinsic_value: Option<Decimal>,
    #[serde(rename = "optionRoot", default)]
    pub option_root: Option<String>,
}

/// One deliverable backing an option contract.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OptionDeliverables {
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(rename = "assetType", default)]
    pub asset_type: Option<String>,
    /// Number of units delivered, sent by Schwab as a string.
    #[serde(rename = "deliverableUnits", default)]
    pub deliverable_units: Option<String>,
    #[serde(rename = "currencyType", default)]
    pub currency_type: Option<String>,
}

// --- Enums ---

string_enum! {
    /// `contractType` query value.
    ContractType {
        Call = "CALL",
        Put = "PUT",
        All = "ALL",
    }
}

string_enum! {
    /// Option chain `strategy`. Used both as a `strategy` query value and
    /// in the [`OptionChain::strategy`] response field.
    OptionStrategy {
        Single = "SINGLE",
        Analytical = "ANALYTICAL",
        Covered = "COVERED",
        Vertical = "VERTICAL",
        Calendar = "CALENDAR",
        Strangle = "STRANGLE",
        Straddle = "STRADDLE",
        Butterfly = "BUTTERFLY",
        Condor = "CONDOR",
        Diagonal = "DIAGONAL",
        Collar = "COLLAR",
        Roll = "ROLL",
    }
}

string_enum! {
    /// `range` query value - the moneyness window of the chain.
    OptionRange {
        /// In the money.
        Itm = "ITM",
        /// Near the money.
        Ntm = "NTM",
        /// Out of the money.
        Otm = "OTM",
        /// Strikes above market.
        Sak = "SAK",
        /// Strikes below market.
        Sbk = "SBK",
        /// Strikes near market.
        Snk = "SNK",
        All = "ALL",
    }
}

string_enum! {
    /// `expMonth` query value.
    ExpirationMonth {
        Jan = "JAN",
        Feb = "FEB",
        Mar = "MAR",
        Apr = "APR",
        May = "MAY",
        Jun = "JUN",
        Jul = "JUL",
        Aug = "AUG",
        Sep = "SEP",
        Oct = "OCT",
        Nov = "NOV",
        Dec = "DEC",
        All = "ALL",
    }
}

string_enum! {
    /// `optionType` query value.
    OptionType {
        Standard = "S",
        NonStandard = "NS",
    }
}

string_enum! {
    /// `entitlement` query value, applicable only to retail tokens.
    Entitlement {
        /// Paying professional.
        PayingPro = "PP",
        /// Non-professional.
        NonPro = "NP",
        /// Non-paying professional.
        NonPayingPro = "PN",
    }
}

string_enum! {
    /// Put/call discriminator on an [`OptionContract`].
    PutCall {
        Put = "PUT",
        Call = "CALL",
    }
}

string_enum! {
    /// Exchange of the [`Underlying`] security.
    UnderlyingExchange {
        Ind = "IND",
        Ase = "ASE",
        Nys = "NYS",
        Nas = "NAS",
        Nap = "NAP",
        Pac = "PAC",
        Opr = "OPR",
        Bats = "BATS",
    }
}

string_enum! {
    /// Option expiration calendar cycle. `M` end-of-month, `Q` quarterly,
    /// `S` standard (3rd-Friday) and `W` weekly.
    ExpirationType {
        EndOfMonth = "M",
        Quarterly = "Q",
        Standard = "S",
        Weekly = "W",
    }
}

string_enum! {
    /// Option contract settlement time.
    SettlementType {
        /// AM settlement.
        Am = "A",
        /// PM settlement.
        Pm = "P",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn option_chain_parses() {
        // Shape modeled on Schwab's documented response: underlying
        // snapshot plus a callExpDateMap with one expiration and one
        // strike. The per-strike value is a list of contracts.
        let json = r#"{
            "symbol": "AAPL",
            "status": "SUCCESS",
            "strategy": "SINGLE",
            "isDelayed": false,
            "isIndex": false,
            "interestRate": 4.85,
            "underlyingPrice": 150.25,
            "volatility": 29.0,
            "underlying": {
                "symbol": "AAPL",
                "description": "Apple Inc",
                "bid": 150.20,
                "ask": 150.30,
                "last": 150.25,
                "exchangeName": "NAS",
                "totalVolume": 50000000,
                "quoteTime": 1710432000000
            },
            "callExpDateMap": {
                "2024-01-19:5": {
                    "150.0": [
                        {
                            "putCall": "CALL",
                            "symbol": "AAPL  240119C00150000",
                            "description": "AAPL 01/19/2024 150.00 C",
                            "bidPrice": 2.10,
                            "askPrice": 2.20,
                            "lastPrice": 2.15,
                            "delta": 0.52,
                            "gamma": 0.08,
                            "theta": -0.04,
                            "vega": 0.11,
                            "rho": 0.02,
                            "openInterest": 12000.0,
                            "strikePrice": 150.0,
                            "expirationDate": "2024-01-19T00:00:00.000+00:00",
                            "daysToExpiration": 5,
                            "expirationType": "W",
                            "settlementType": "P",
                            "lastTradingDay": 1705622400000,
                            "isInTheMoney": true,
                            "totalVolume": 3400
                        }
                    ]
                }
            },
            "putExpDateMap": {}
        }"#;
        let chain: OptionChain = serde_json::from_str(json).unwrap();
        assert_eq!(chain.symbol.as_deref(), Some("AAPL"));
        assert_eq!(chain.strategy, Some(OptionStrategy::Single));
        assert_eq!(chain.underlying_price, Some(dec!(150.25)));

        let underlying = chain.underlying.as_ref().unwrap();
        assert_eq!(underlying.exchange_name, Some(UnderlyingExchange::Nas));
        assert_eq!(underlying.total_volume, Some(50000000));

        let exp = chain.call_exp_date_map.get("2024-01-19:5").unwrap();
        let strike = exp.get("150.0").unwrap();
        assert_eq!(strike.len(), 1);

        let contract = &strike[0];
        assert_eq!(contract.put_call, Some(PutCall::Call));
        assert_eq!(contract.bid_price, Some(dec!(2.10)));
        assert_eq!(contract.delta, Some(dec!(0.52)));
        assert_eq!(contract.open_interest, Some(dec!(12000.0)));
        assert_eq!(contract.strike_price, Some(dec!(150.0)));
        assert_eq!(contract.days_to_expiration, Some(5));
        assert_eq!(contract.expiration_type, Some(ExpirationType::Weekly));
        assert_eq!(contract.settlement_type, Some(SettlementType::Pm));
        assert_eq!(contract.last_trading_day, Some(1705622400000));
        assert_eq!(contract.is_in_the_money, Some(true));

        assert!(chain.put_exp_date_map.is_empty());
    }

    #[test]
    fn per_strike_array_form_is_accepted() {
        // Array shape: the per-strike value is a list of contracts. This
        // fixture carries two contracts at one strike to confirm the full
        // list is retained.
        let json = r#"{
            "callExpDateMap": {
                "2024-01-19:5": {
                    "150.0": [
                        { "symbol": "AAPL  240119C00150000", "putCall": "CALL" },
                        { "symbol": "AAPL  240119C00150000-MINI", "putCall": "CALL", "isMini": true }
                    ]
                }
            }
        }"#;
        let chain: OptionChain = serde_json::from_str(json).unwrap();
        let strike = chain
            .call_exp_date_map
            .get("2024-01-19:5")
            .unwrap()
            .get("150.0")
            .unwrap();
        assert_eq!(strike.len(), 2);
        assert_eq!(strike[1].is_mini, Some(true));
    }

    #[test]
    fn per_strike_single_object_form_is_accepted() {
        // Single-object shape (Schwab's published schema): the per-strike
        // value is one contract rather than an array. It is normalized to
        // a one-element list. Both maps are exercised to confirm each
        // field uses the tolerant path.
        let json = r#"{
            "callExpDateMap": {
                "2024-01-19:5": {
                    "150.0": { "symbol": "AAPL  240119C00150000", "putCall": "CALL", "bidPrice": 2.10 }
                }
            },
            "putExpDateMap": {
                "2024-01-19:5": {
                    "150.0": { "symbol": "AAPL  240119P00150000", "putCall": "PUT" }
                }
            }
        }"#;
        let chain: OptionChain = serde_json::from_str(json).unwrap();

        let call = chain
            .call_exp_date_map
            .get("2024-01-19:5")
            .unwrap()
            .get("150.0")
            .unwrap();
        assert_eq!(call.len(), 1);
        assert_eq!(call[0].bid_price, Some(dec!(2.10)));

        let put = chain
            .put_exp_date_map
            .get("2024-01-19:5")
            .unwrap()
            .get("150.0")
            .unwrap();
        assert_eq!(put.len(), 1);
        assert_eq!(put[0].put_call, Some(PutCall::Put));
    }

    #[test]
    fn empty_option_chain_parses() {
        let chain: OptionChain = serde_json::from_str("{}").unwrap();
        assert!(chain.call_exp_date_map.is_empty());
        assert!(chain.put_exp_date_map.is_empty());
        assert!(chain.underlying.is_none());
    }

    #[test]
    fn contract_type_round_trips_known_variants() {
        for raw in ["CALL", "PUT", "ALL"] {
            let json = format!(r#""{raw}""#);
            let parsed: ContractType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn option_strategy_round_trips_known_variants() {
        for raw in [
            "SINGLE",
            "ANALYTICAL",
            "COVERED",
            "VERTICAL",
            "CALENDAR",
            "STRANGLE",
            "STRADDLE",
            "BUTTERFLY",
            "CONDOR",
            "DIAGONAL",
            "COLLAR",
            "ROLL",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: OptionStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn expiration_type_round_trips_single_letter_codes() {
        for raw in ["M", "Q", "S", "W"] {
            let json = format!(r#""{raw}""#);
            let parsed: ExpirationType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn settlement_type_round_trips_single_letter_codes() {
        for raw in ["A", "P"] {
            let json = format!(r#""{raw}""#);
            let parsed: SettlementType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn unknown_option_strategy_preserves_raw_string() {
        let parsed: OptionStrategy = serde_json::from_str(r#""IRON_CONDOR""#).unwrap();
        assert!(matches!(parsed, OptionStrategy::Unknown(ref s) if s == "IRON_CONDOR"));
    }
}
