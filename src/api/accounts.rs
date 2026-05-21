//! `GET /accounts` family - Schwab Trader API.
//!
//! Endpoints:
//!
//! - `GET /accounts/accountNumbers` returns the plain -> encrypted-hash
//!   mapping. Every other Trader API path takes the encrypted hash, not the
//!   plain account number, in its `{accountNumber}` segment.
//! - `GET /accounts` returns balances (and optionally positions) for every
//!   linked account.
//! - `GET /accounts/{accountNumber}` returns the same shape for a single
//!   account, keyed by the encrypted hash.
//!
//! Reached through [`SchwabClient::accounts`](crate::SchwabClient::accounts).

use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::model::{AccountHash, AccountNumber};
use crate::rest::SchwabClient;

/// Accessor for the `/accounts*` endpoint family. Construct via
/// [`SchwabClient::accounts`].
pub struct Accounts<'a> {
    client: &'a SchwabClient,
}

impl<'a> Accounts<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// `GET /accounts/accountNumbers` - plain-account-number to
    /// encrypted-hash mapping. The hash is what subsequent endpoints
    /// require in the `{accountNumber}` URL path segment.
    pub async fn numbers(&self) -> Result<Vec<AccountNumberHash>> {
        self.client.get_json("/accounts/accountNumbers").await
    }

    /// Begin a `GET /accounts` request. Defaults to balances only; call
    /// [`ListAccountsBuilder::with_positions`] to include positions.
    /// Terminate with [`ListAccountsBuilder::send`].
    pub fn list(&self) -> ListAccountsBuilder<'a> {
        ListAccountsBuilder {
            client: self.client,
            include_positions: false,
        }
    }

    /// Begin a `GET /accounts/{accountNumber}` request for a single account.
    /// `account_hash` is the encrypted value from [`Self::numbers`], never
    /// the plain account number.
    pub fn get<'b>(&self, account_hash: &'b AccountHash) -> GetAccountBuilder<'a, 'b> {
        GetAccountBuilder {
            client: self.client,
            account_hash,
            include_positions: false,
        }
    }
}

/// In-flight request for `GET /accounts`. Built via [`Accounts::list`].
#[must_use = "call .send() to execute the request"]
pub struct ListAccountsBuilder<'a> {
    client: &'a SchwabClient,
    include_positions: bool,
}

impl<'a> ListAccountsBuilder<'a> {
    /// Add `fields=positions` to the query string. Without this, the
    /// response carries balances only.
    pub fn with_positions(mut self) -> Self {
        self.include_positions = true;
        self
    }

    pub async fn send(self) -> Result<Vec<Account>> {
        let path = if self.include_positions {
            "/accounts?fields=positions"
        } else {
            "/accounts"
        };
        self.client.get_json(path).await
    }
}

/// In-flight request for `GET /accounts/{accountNumber}`. Built via
/// [`Accounts::get`].
#[must_use = "call .send() to execute the request"]
pub struct GetAccountBuilder<'a, 'b> {
    client: &'a SchwabClient,
    account_hash: &'b AccountHash,
    include_positions: bool,
}

impl<'a, 'b> GetAccountBuilder<'a, 'b> {
    /// Add `fields=positions` to the query string. Without this, the
    /// response carries balances only.
    pub fn with_positions(mut self) -> Self {
        self.include_positions = true;
        self
    }

    pub async fn send(self) -> Result<Account> {
        let hash = self.account_hash.expose_secret();
        let path = if self.include_positions {
            format!("/accounts/{hash}?fields=positions")
        } else {
            format!("/accounts/{hash}")
        };
        self.client.get_json(&path).await
    }
}

/// `GET /accounts/accountNumbers` response item.
///
/// `account_number` is the plain number (PII). `hash_value` is the opaque
/// identifier required in the `{accountNumber}` path segment of every other
/// Trader API endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountNumberHash {
    #[serde(rename = "accountNumber")]
    pub account_number: AccountNumber,
    #[serde(rename = "hashValue")]
    pub hash_value: AccountHash,
}

/// `GET /accounts` / `GET /accounts/{accountNumber}` response envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    #[serde(rename = "securitiesAccount")]
    pub securities_account: SecuritiesAccount,
}

/// Margin or cash account. Discriminated by the wire `type` field.
///
/// Both variants carry many `Option<Decimal>` balance fields, so the enum is
/// around 1.5 KB; the response is allocated on the heap by reqwest's JSON
/// decoder, so the enum-size warning is irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum SecuritiesAccount {
    #[serde(rename = "MARGIN")]
    Margin(MarginAccount),
    #[serde(rename = "CASH")]
    Cash(CashAccount),
}

impl SecuritiesAccount {
    pub fn account_number(&self) -> &AccountNumber {
        match self {
            SecuritiesAccount::Margin(a) => &a.account_number,
            SecuritiesAccount::Cash(a) => &a.account_number,
        }
    }

    pub fn positions(&self) -> &[Position] {
        match self {
            SecuritiesAccount::Margin(a) => &a.positions,
            SecuritiesAccount::Cash(a) => &a.positions,
        }
    }

    pub fn is_day_trader(&self) -> bool {
        match self {
            SecuritiesAccount::Margin(a) => a.is_day_trader,
            SecuritiesAccount::Cash(a) => a.is_day_trader,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarginAccount {
    /// Plain account number. The `{accountNumber}` URL path segment uses the
    /// encrypted [`AccountHash`] instead.
    #[serde(rename = "accountNumber")]
    pub account_number: AccountNumber,
    #[serde(rename = "roundTrips", default)]
    pub round_trips: i32,
    #[serde(rename = "isDayTrader", default)]
    pub is_day_trader: bool,
    #[serde(rename = "isClosingOnlyRestricted", default)]
    pub is_closing_only_restricted: bool,
    #[serde(rename = "pfcbFlag", default)]
    pub pfcb_flag: bool,
    /// Empty unless the request included `fields=positions`.
    #[serde(default)]
    pub positions: Vec<Position>,
    #[serde(rename = "initialBalances", default)]
    pub initial_balances: Option<MarginInitialBalance>,
    #[serde(rename = "currentBalances", default)]
    pub current_balances: Option<MarginBalance>,
    #[serde(rename = "projectedBalances", default)]
    pub projected_balances: Option<MarginBalance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CashAccount {
    /// Plain account number. The `{accountNumber}` URL path segment uses the
    /// encrypted [`AccountHash`] instead.
    #[serde(rename = "accountNumber")]
    pub account_number: AccountNumber,
    #[serde(rename = "roundTrips", default)]
    pub round_trips: i32,
    #[serde(rename = "isDayTrader", default)]
    pub is_day_trader: bool,
    #[serde(rename = "isClosingOnlyRestricted", default)]
    pub is_closing_only_restricted: bool,
    #[serde(rename = "pfcbFlag", default)]
    pub pfcb_flag: bool,
    /// Empty unless the request included `fields=positions`.
    #[serde(default)]
    pub positions: Vec<Position>,
    #[serde(rename = "initialBalances", default)]
    pub initial_balances: Option<CashInitialBalance>,
    #[serde(rename = "currentBalances", default)]
    pub current_balances: Option<CashBalance>,
    #[serde(rename = "projectedBalances", default)]
    pub projected_balances: Option<CashBalance>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MarginInitialBalance {
    #[serde(default, with = "decimal_opt", rename = "accruedInterest")]
    pub accrued_interest: Option<Decimal>,
    #[serde(
        default,
        with = "decimal_opt",
        rename = "availableFundsNonMarginableTrade"
    )]
    pub available_funds_non_marginable_trade: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "bondValue")]
    pub bond_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "buyingPower")]
    pub buying_power: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashBalance")]
    pub cash_balance: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForTrading")]
    pub cash_available_for_trading: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashReceipts")]
    pub cash_receipts: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPower")]
    pub day_trading_buying_power: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPowerCall")]
    pub day_trading_buying_power_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "dayTradingEquityCall")]
    pub day_trading_equity_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub equity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "equityPercentage")]
    pub equity_percentage: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "liquidationValue")]
    pub liquidation_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longMarginValue")]
    pub long_margin_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longOptionMarketValue")]
    pub long_option_market_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longStockValue")]
    pub long_stock_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "maintenanceCall")]
    pub maintenance_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "maintenanceRequirement")]
    pub maintenance_requirement: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub margin: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "marginEquity")]
    pub margin_equity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "moneyMarketFund")]
    pub money_market_fund: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "mutualFundValue")]
    pub mutual_fund_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "regTCall")]
    pub reg_t_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortMarginValue")]
    pub short_margin_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortOptionMarketValue")]
    pub short_option_market_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortStockValue")]
    pub short_stock_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "totalCash")]
    pub total_cash: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "isInCall")]
    pub is_in_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "unsettledCash")]
    pub unsettled_cash: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "pendingDeposits")]
    pub pending_deposits: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "marginBalance")]
    pub margin_balance: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortBalance")]
    pub short_balance: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "accountValue")]
    pub account_value: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MarginBalance {
    #[serde(default, with = "decimal_opt", rename = "availableFunds")]
    pub available_funds: Option<Decimal>,
    #[serde(
        default,
        with = "decimal_opt",
        rename = "availableFundsNonMarginableTrade"
    )]
    pub available_funds_non_marginable_trade: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "buyingPower")]
    pub buying_power: Option<Decimal>,
    #[serde(
        default,
        with = "decimal_opt",
        rename = "buyingPowerNonMarginableTrade"
    )]
    pub buying_power_non_marginable_trade: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPower")]
    pub day_trading_buying_power: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPowerCall")]
    pub day_trading_buying_power_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub equity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "equityPercentage")]
    pub equity_percentage: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longMarginValue")]
    pub long_margin_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "maintenanceCall")]
    pub maintenance_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "maintenanceRequirement")]
    pub maintenance_requirement: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "marginBalance")]
    pub margin_balance: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "regTCall")]
    pub reg_t_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortBalance")]
    pub short_balance: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortMarginValue")]
    pub short_margin_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub sma: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "isInCall")]
    pub is_in_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "stockBuyingPower")]
    pub stock_buying_power: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "optionBuyingPower")]
    pub option_buying_power: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CashInitialBalance {
    #[serde(default, with = "decimal_opt", rename = "accruedInterest")]
    pub accrued_interest: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForTrading")]
    pub cash_available_for_trading: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForWithdrawal")]
    pub cash_available_for_withdrawal: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashBalance")]
    pub cash_balance: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "bondValue")]
    pub bond_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashReceipts")]
    pub cash_receipts: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "liquidationValue")]
    pub liquidation_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longOptionMarketValue")]
    pub long_option_market_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longStockValue")]
    pub long_stock_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "moneyMarketFund")]
    pub money_market_fund: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "mutualFundValue")]
    pub mutual_fund_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortOptionMarketValue")]
    pub short_option_market_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortStockValue")]
    pub short_stock_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "isInCall")]
    pub is_in_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "unsettledCash")]
    pub unsettled_cash: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashDebitCallValue")]
    pub cash_debit_call_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "pendingDeposits")]
    pub pending_deposits: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "accountValue")]
    pub account_value: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CashBalance {
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForTrading")]
    pub cash_available_for_trading: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForWithdrawal")]
    pub cash_available_for_withdrawal: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashCall")]
    pub cash_call: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longNonMarginableMarketValue")]
    pub long_non_marginable_market_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "totalCash")]
    pub total_cash: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "cashDebitCallValue")]
    pub cash_debit_call_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "unsettledCash")]
    pub unsettled_cash: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Position {
    #[serde(default, with = "decimal_opt", rename = "shortQuantity")]
    pub short_quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "averagePrice")]
    pub average_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "currentDayProfitLoss")]
    pub current_day_profit_loss: Option<Decimal>,
    #[serde(
        default,
        with = "decimal_opt",
        rename = "currentDayProfitLossPercentage"
    )]
    pub current_day_profit_loss_percentage: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longQuantity")]
    pub long_quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "settledLongQuantity")]
    pub settled_long_quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "settledShortQuantity")]
    pub settled_short_quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "agedQuantity")]
    pub aged_quantity: Option<Decimal>,
    #[serde(default)]
    pub instrument: Option<AccountsInstrument>,
    #[serde(default, with = "decimal_opt", rename = "marketValue")]
    pub market_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "maintenanceRequirement")]
    pub maintenance_requirement: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "averageLongPrice")]
    pub average_long_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "averageShortPrice")]
    pub average_short_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "taxLotAverageLongPrice")]
    pub tax_lot_average_long_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "taxLotAverageShortPrice")]
    pub tax_lot_average_short_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "longOpenProfitLoss")]
    pub long_open_profit_loss: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "shortOpenProfitLoss")]
    pub short_open_profit_loss: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "previousSessionLongQuantity")]
    pub previous_session_long_quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "previousSessionShortQuantity")]
    pub previous_session_short_quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "currentDayCost")]
    pub current_day_cost: Option<Decimal>,
}

/// Instrument carried by a `Position`. Flat struct: every field that exists
/// on any documented asset variant lives here as `Option`, so newly-added
/// asset types deserialize cleanly even if this crate has not been updated.
/// Consumers match on [`AccountsInstrument::asset_type`] to route.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountsInstrument {
    #[serde(rename = "assetType")]
    pub asset_type: AssetType,
    #[serde(default)]
    pub cusip: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "instrumentId")]
    pub instrument_id: Option<i64>,
    #[serde(default, with = "decimal_opt", rename = "netChange")]
    pub net_change: Option<Decimal>,

    // Option-specific fields. `None` on non-option asset types.
    #[serde(default, rename = "optionDeliverables")]
    pub option_deliverables: Vec<AccountApiOptionDeliverable>,
    #[serde(default, rename = "putCall")]
    pub put_call: Option<PutCall>,
    #[serde(default, rename = "optionMultiplier")]
    pub option_multiplier: Option<i32>,
    #[serde(default, rename = "type")]
    pub option_type: Option<OptionType>,
    #[serde(default, rename = "underlyingSymbol")]
    pub underlying_symbol: Option<String>,

    // Fixed-income-specific fields. `None` on non-fixed-income asset types.
    // Schwab ships ISO-8601 timestamps as strings; date parsing is left to
    // the consumer for now (no chrono dep on this crate yet).
    #[serde(default, rename = "maturityDate")]
    pub maturity_date: Option<String>,
    #[serde(default, with = "decimal_opt")]
    pub factor: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "variableRate")]
    pub variable_rate: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountApiOptionDeliverable {
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default, with = "decimal_opt", rename = "deliverableUnits")]
    pub deliverable_units: Option<Decimal>,
    #[serde(default, rename = "apiCurrencyType")]
    pub currency_type: Option<ApiCurrencyType>,
    #[serde(default, rename = "assetType")]
    pub asset_type: Option<AssetType>,
}

/// Schwab `assetType` discriminator. Includes a catch-all so wire values
/// added after this crate was published deserialize as `Unknown(raw)`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString, Serialize, Deserialize,
)]
#[serde(into = "String", from = "String")]
pub enum AssetType {
    #[strum(serialize = "EQUITY")]
    Equity,
    #[strum(serialize = "MUTUAL_FUND")]
    MutualFund,
    #[strum(serialize = "OPTION")]
    Option,
    #[strum(serialize = "FUTURE")]
    Future,
    #[strum(serialize = "FOREX")]
    Forex,
    #[strum(serialize = "INDEX")]
    Index,
    #[strum(serialize = "CASH_EQUIVALENT")]
    CashEquivalent,
    #[strum(serialize = "FIXED_INCOME")]
    FixedIncome,
    #[strum(serialize = "PRODUCT")]
    Product,
    #[strum(serialize = "CURRENCY")]
    Currency,
    #[strum(serialize = "COLLECTIVE_INVESTMENT")]
    CollectiveInvestment,
    #[strum(default)]
    Unknown(String),
}

impl Default for AssetType {
    fn default() -> Self {
        AssetType::Unknown(String::new())
    }
}

impl From<AssetType> for String {
    fn from(value: AssetType) -> Self {
        value.to_string()
    }
}

impl From<String> for AssetType {
    fn from(value: String) -> Self {
        value
            .parse()
            .expect("AssetType FromStr is infallible (strum default)")
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString, Serialize, Deserialize,
)]
#[serde(into = "String", from = "String")]
pub enum PutCall {
    #[strum(serialize = "PUT")]
    Put,
    #[strum(serialize = "CALL")]
    Call,
    #[strum(serialize = "UNKNOWN")]
    UnknownSchwab,
    #[strum(default)]
    Unknown(String),
}

impl From<PutCall> for String {
    fn from(value: PutCall) -> Self {
        value.to_string()
    }
}

impl From<String> for PutCall {
    fn from(value: String) -> Self {
        value
            .parse()
            .expect("PutCall FromStr is infallible (strum default)")
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString, Serialize, Deserialize,
)]
#[serde(into = "String", from = "String")]
pub enum OptionType {
    #[strum(serialize = "VANILLA")]
    Vanilla,
    #[strum(serialize = "BINARY")]
    Binary,
    #[strum(serialize = "BARRIER")]
    Barrier,
    #[strum(serialize = "UNKNOWN")]
    UnknownSchwab,
    #[strum(default)]
    Unknown(String),
}

impl From<OptionType> for String {
    fn from(value: OptionType) -> Self {
        value.to_string()
    }
}

impl From<String> for OptionType {
    fn from(value: String) -> Self {
        value
            .parse()
            .expect("OptionType FromStr is infallible (strum default)")
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString, Serialize, Deserialize,
)]
#[serde(into = "String", from = "String")]
pub enum ApiCurrencyType {
    #[strum(serialize = "USD")]
    Usd,
    #[strum(serialize = "CAD")]
    Cad,
    #[strum(serialize = "EUR")]
    Eur,
    #[strum(serialize = "JPY")]
    Jpy,
    #[strum(default)]
    Unknown(String),
}

impl From<ApiCurrencyType> for String {
    fn from(value: ApiCurrencyType) -> Self {
        value.to_string()
    }
}

impl From<String> for ApiCurrencyType {
    fn from(value: String) -> Self {
        value
            .parse()
            .expect("ApiCurrencyType FromStr is infallible (strum default)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn account_number_hash_parses() {
        let json = r#"{"accountNumber":"12345678","hashValue":"ABCDEF1234567890"}"#;
        let parsed: AccountNumberHash = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.account_number.expose_secret(), "12345678");
        assert_eq!(parsed.hash_value.expose_secret(), "ABCDEF1234567890");
    }

    #[test]
    fn margin_account_parses() {
        let json = r#"{
            "securitiesAccount": {
                "type": "MARGIN",
                "accountNumber": "12345678",
                "roundTrips": 0,
                "isDayTrader": false,
                "isClosingOnlyRestricted": false,
                "pfcbFlag": false,
                "currentBalances": {
                    "availableFunds": 1000.50,
                    "buyingPower": 2000.00,
                    "equity": 5000.25
                }
            }
        }"#;
        let parsed: Account = serde_json::from_str(json).unwrap();
        let margin = match &parsed.securities_account {
            SecuritiesAccount::Margin(m) => m,
            other => panic!("expected MARGIN, got {other:?}"),
        };
        assert_eq!(margin.account_number.expose_secret(), "12345678");
        let balances = margin.current_balances.as_ref().unwrap();
        assert_eq!(balances.available_funds, Some(dec!(1000.50)));
        assert_eq!(balances.equity, Some(dec!(5000.25)));
        // Untouched balance fields are `None`, not zero, so a consumer can
        // tell "Schwab did not send this" from "Schwab sent 0".
        assert_eq!(balances.buying_power_non_marginable_trade, None);
    }

    #[test]
    fn cash_account_parses() {
        let json = r#"{
            "securitiesAccount": {
                "type": "CASH",
                "accountNumber": "87654321",
                "roundTrips": 0,
                "isDayTrader": false,
                "isClosingOnlyRestricted": false,
                "pfcbFlag": false,
                "currentBalances": {
                    "cashAvailableForTrading": 500.00,
                    "totalCash": 500.00
                }
            }
        }"#;
        let parsed: Account = serde_json::from_str(json).unwrap();
        let cash = match &parsed.securities_account {
            SecuritiesAccount::Cash(c) => c,
            other => panic!("expected CASH, got {other:?}"),
        };
        assert_eq!(cash.account_number.expose_secret(), "87654321");
        let balances = cash.current_balances.as_ref().unwrap();
        assert_eq!(balances.cash_available_for_trading, Some(dec!(500.00)));
    }

    #[test]
    fn account_with_equity_position() {
        let json = r#"{
            "securitiesAccount": {
                "type": "MARGIN",
                "accountNumber": "11111111",
                "positions": [{
                    "longQuantity": 10,
                    "averagePrice": 145.32,
                    "marketValue": 1500.00,
                    "instrument": {
                        "assetType": "EQUITY",
                        "symbol": "AAPL",
                        "cusip": "037833100",
                        "description": "Apple Inc",
                        "instrumentId": 12345
                    }
                }]
            }
        }"#;
        let parsed: Account = serde_json::from_str(json).unwrap();
        let positions = parsed.securities_account.positions();
        assert_eq!(positions.len(), 1);
        let pos = &positions[0];
        assert_eq!(pos.long_quantity, Some(dec!(10)));
        assert_eq!(pos.average_price, Some(dec!(145.32)));
        let inst = pos.instrument.as_ref().unwrap();
        assert_eq!(inst.asset_type, AssetType::Equity);
        assert_eq!(inst.symbol.as_deref(), Some("AAPL"));
        assert_eq!(inst.instrument_id, Some(12345));
    }

    #[test]
    fn account_with_option_position() {
        let json = r#"{
            "securitiesAccount": {
                "type": "MARGIN",
                "accountNumber": "11111111",
                "positions": [{
                    "longQuantity": 1,
                    "averagePrice": 6.45,
                    "instrument": {
                        "assetType": "OPTION",
                        "symbol": "AAPL  240315C00200000",
                        "underlyingSymbol": "AAPL",
                        "putCall": "CALL",
                        "type": "VANILLA",
                        "optionMultiplier": 100,
                        "optionDeliverables": [{
                            "symbol": "AAPL",
                            "deliverableUnits": 100,
                            "apiCurrencyType": "USD",
                            "assetType": "EQUITY"
                        }]
                    }
                }]
            }
        }"#;
        let parsed: Account = serde_json::from_str(json).unwrap();
        let pos = &parsed.securities_account.positions()[0];
        let inst = pos.instrument.as_ref().unwrap();
        assert_eq!(inst.asset_type, AssetType::Option);
        assert_eq!(inst.put_call, Some(PutCall::Call));
        assert_eq!(inst.option_type, Some(OptionType::Vanilla));
        assert_eq!(inst.option_multiplier, Some(100));
        assert_eq!(inst.option_deliverables.len(), 1);
        let deliv = &inst.option_deliverables[0];
        assert_eq!(deliv.currency_type, Some(ApiCurrencyType::Usd));
        assert_eq!(deliv.deliverable_units, Some(dec!(100)));
    }

    #[test]
    fn unknown_asset_type_preserves_raw_string() {
        let json =
            r#"{"assetType":"NEW_ASSET_KIND","symbol":"WHAT","description":"Tomorrows thing"}"#;
        let inst: AccountsInstrument = serde_json::from_str(json).unwrap();
        match &inst.asset_type {
            AssetType::Unknown(raw) => assert_eq!(raw, "NEW_ASSET_KIND"),
            other => panic!("expected Unknown, got {other:?}"),
        }
        // The strum enum itself round-trips - that is what guarantees a
        // consumer matching on the variant gets the raw discriminator back.
        let just_asset: AssetType = serde_json::from_str(r#""NEW_ASSET_KIND""#).unwrap();
        assert_eq!(
            serde_json::to_string(&just_asset).unwrap(),
            r#""NEW_ASSET_KIND""#
        );
    }

    #[test]
    fn empty_positions_field_omitted() {
        let json = r#"{
            "securitiesAccount": {
                "type": "MARGIN",
                "accountNumber": "12345"
            }
        }"#;
        let parsed: Account = serde_json::from_str(json).unwrap();
        assert!(parsed.securities_account.positions().is_empty());
    }

    #[test]
    fn asset_type_round_trips_each_known_variant() {
        for raw in [
            "EQUITY",
            "MUTUAL_FUND",
            "OPTION",
            "FUTURE",
            "FOREX",
            "INDEX",
            "CASH_EQUIVALENT",
            "FIXED_INCOME",
            "PRODUCT",
            "CURRENCY",
            "COLLECTIVE_INVESTMENT",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: AssetType = serde_json::from_str(&json).unwrap();
            let serialized = serde_json::to_string(&parsed).unwrap();
            assert_eq!(serialized, json, "round trip failed for {raw}");
        }
    }
}
