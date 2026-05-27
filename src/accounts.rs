//! `GET /accounts` family
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
//! See [`Accounts`] for available methods.
//!
//! # Examples
//!
//! ## List linked accounts with their positions
//!
//! Balances are returned by default. Positions are opt-in via
//! [`ListAccountsBuilder::with_positions`].
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient};
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! let accounts = client.accounts().list().with_positions().send().await?;
//! for account in &accounts {
//!     let acct = &account.securities_account;
//!     println!("{} ({} positions)", acct.account_type(), acct.positions().len());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Read balances off a single account
//!
//! You'll need the encrypted [`AccountHash`] from [`Accounts::numbers`] to
//! query the account. Balance fields are `Option<Decimal>`, where `None` means
//! Schwab omitted the field, distinct from a sent zero.
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient};
//! use schwab_sdk::accounts::SecuritiesAccount;
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! // The plain -> encrypted-hash mapping. The hash is used for other endpoints.
//! let accounts = client.accounts().numbers().await?;
//! let account_hash = &accounts.first().expect("a linked account").hash_value;
//!
//! // Get the account and read the balances.
//! let account = client.accounts().get(account_hash).with_positions().send().await?;
//! if let SecuritiesAccount::Margin(margin) = &account.securities_account {
//!     if let Some(balances) = &margin.current_balances {
//!         println!("buying power: {:?}", balances.buying_power);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;
use crate::secrets::{AccountHash, AccountNumber};

/// Accessor for the `/accounts*` endpoint family. Construct via
/// [`SchwabClient::accounts`].
#[derive(Debug)]
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
        self.client
            .trader_http()
            .get_json("/accounts/accountNumbers")
            .await
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
#[derive(Debug)]
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

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Account>> {
        let path = if self.include_positions {
            "/accounts?fields=positions"
        } else {
            "/accounts"
        };
        self.client.trader_http().get_json(path).await
    }
}

/// In-flight request for `GET /accounts/{accountNumber}`. Built via
/// [`Accounts::get`].
#[derive(Debug)]
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

    /// Execute the request.
    pub async fn send(self) -> Result<Account> {
        let hash = self.account_hash.expose_secret();
        let path = if self.include_positions {
            format!("/accounts/{hash}?fields=positions")
        } else {
            format!("/accounts/{hash}")
        };
        self.client.trader_http().get_json(&path).await
    }
}

/// `GET /accounts/accountNumbers` response item.
///
/// `account_number` is the plain number (PII). `hash_value` is the opaque
/// identifier required in the `{accountNumber}` path segment of every other
/// Trader API endpoint.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct AccountNumberHash {
    /// Plain account number (PII).
    #[serde(rename = "accountNumber")]
    pub account_number: AccountNumber,
    /// Encrypted hash used as the `{accountNumber}` path segment on every
    /// other Trader API endpoint.
    #[serde(rename = "hashValue")]
    pub hash_value: AccountHash,
}

/// `GET /accounts` / `GET /accounts/{accountNumber}` response envelope.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Account {
    /// Account-specific balances, positions, and identifiers.
    #[serde(rename = "securitiesAccount")]
    pub securities_account: SecuritiesAccount,
}

/// Margin or cash account. Discriminated by the wire `type` field.
///
/// Account types Schwab adds after this crate was published land in
/// [`SecuritiesAccount::Unknown`] with the raw JSON preserved, so a new
/// `type` value never fails the whole response. Reconciliation code should
/// treat [`SecuritiesAccount::Unknown`] as an unreconcilable account and
/// surface it for manual review rather than making decisions on the partial
/// data the accessors can offer.
///
/// Both `Margin` and `Cash` carry many `Option<Decimal>` balance fields, so
/// the enum is around 1.5 KB; the response is allocated on the heap by
/// reqwest's JSON decoder, so the enum-size warning is irrelevant.
///
/// # Examples
///
/// Route on the account type. The `Unknown` arm should be surfaced for
/// review rather than acted on, since the typed accessors can only offer
/// partial data for it.
///
/// ```no_run
/// use schwab_sdk::accounts::{Account, SecuritiesAccount};
///
/// # fn handle(account: &Account) {
/// match &account.securities_account {
///     SecuritiesAccount::Margin(m) => println!("margin: {} positions", m.positions.len()),
///     SecuritiesAccount::Cash(c) => println!("cash: {} positions", c.positions.len()),
///     SecuritiesAccount::Unknown { account_type, .. } => {
///         // Do not guess. Flag for manual review.
///         eprintln!("unrecognized account type: {account_type}");
///     }
///     // `SecuritiesAccount` is non-exhaustive; treat anything new like Unknown.
///     _ => eprintln!("unrecognized account type"),
/// }
/// # }
/// ```
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SecuritiesAccount {
    /// Margin account (`type: "MARGIN"`).
    Margin(MarginAccount),
    /// Cash account (`type: "CASH"`).
    Cash(CashAccount),
    /// Account `type` Schwab returned that this crate does not recognize.
    /// `account_type` is the raw discriminator string; `raw` is the full
    /// `securitiesAccount` object as Schwab sent it, for diagnostics.
    Unknown {
        /// Raw `type` discriminator Schwab sent.
        account_type: String,
        /// Full `securitiesAccount` object as JSON for diagnostics.
        raw: serde_json::Value,
    },
}

impl<'de> Deserialize<'de> for SecuritiesAccount {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Buffer the full object, then dispatch on `type`. Mirrors the
        // pattern used by [`QuoteEntry`](crate::market_data::quotes::QuoteEntry):
        // unknown discriminators never abort the parse.
        let value = serde_json::Value::deserialize(deserializer)?;
        let account_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde::de::Error::missing_field("type"))?
            .to_string();
        match account_type.as_str() {
            "MARGIN" => MarginAccount::deserialize(value)
                .map(SecuritiesAccount::Margin)
                .map_err(serde::de::Error::custom),
            "CASH" => CashAccount::deserialize(value)
                .map(SecuritiesAccount::Cash)
                .map_err(serde::de::Error::custom),
            _ => Ok(SecuritiesAccount::Unknown {
                account_type,
                raw: value,
            }),
        }
    }
}

impl SecuritiesAccount {
    /// The wire `type` discriminator. `"MARGIN"`, `"CASH"`, or whatever
    /// string Schwab sent for a future account type.
    pub fn account_type(&self) -> &str {
        match self {
            SecuritiesAccount::Margin(_) => "MARGIN",
            SecuritiesAccount::Cash(_) => "CASH",
            SecuritiesAccount::Unknown { account_type, .. } => account_type,
        }
    }

    /// The account number.
    ///
    /// Unrecognized account types return `None`.
    pub fn account_number(&self) -> Option<&AccountNumber> {
        match self {
            SecuritiesAccount::Margin(a) => Some(&a.account_number),
            SecuritiesAccount::Cash(a) => Some(&a.account_number),
            SecuritiesAccount::Unknown { .. } => None,
        }
    }

    /// The positions for the account.
    ///
    /// Unrecognized account types return an empty slice.
    pub fn positions(&self) -> &[Position] {
        match self {
            SecuritiesAccount::Margin(a) => &a.positions,
            SecuritiesAccount::Cash(a) => &a.positions,
            SecuritiesAccount::Unknown { .. } => &[],
        }
    }

    /// Whether the account is a pattern day trader.
    ///
    /// Unrecognized account types return `None`. Silent `false` would be
    /// dangerous in a trading context (it would let PDT-sensitive logic
    /// run against an account whose status is genuinely not known), so the
    /// caller is forced to decide.
    pub fn is_day_trader(&self) -> Option<bool> {
        match self {
            SecuritiesAccount::Margin(a) => Some(a.is_day_trader),
            SecuritiesAccount::Cash(a) => Some(a.is_day_trader),
            SecuritiesAccount::Unknown { .. } => None,
        }
    }
}

/// Margin account body inside a [`SecuritiesAccount::Margin`].
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct MarginAccount {
    /// Plain account number. The `{accountNumber}` URL path segment uses the
    /// encrypted [`AccountHash`] instead.
    #[serde(rename = "accountNumber")]
    pub account_number: AccountNumber,
    /// Round-trip count used for PDT classification.
    #[serde(rename = "roundTrips", default)]
    pub round_trips: i32,
    /// `true` if Schwab has flagged this account as a pattern day trader.
    #[serde(rename = "isDayTrader", default)]
    pub is_day_trader: bool,
    /// `true` if Schwab has restricted the account to closing-only trades.
    #[serde(rename = "isClosingOnlyRestricted", default)]
    pub is_closing_only_restricted: bool,
    /// Schwab `pfcbFlag` (post free credit balance) indicator.
    #[serde(rename = "pfcbFlag", default)]
    pub pfcb_flag: bool,
    /// Empty unless the request included `fields=positions`.
    #[serde(default)]
    pub positions: Vec<Position>,
    /// Balances at the start of the trading day, before any session activity.
    #[serde(rename = "initialBalances", default)]
    pub initial_balances: Option<MarginInitialBalance>,
    /// Balances reflecting all settled activity to date.
    #[serde(rename = "currentBalances", default)]
    pub current_balances: Option<MarginBalance>,
    /// Balances Schwab projects after pending (unsettled) activity clears.
    #[serde(rename = "projectedBalances", default)]
    pub projected_balances: Option<MarginBalance>,
}

/// Cash account body inside a [`SecuritiesAccount::Cash`].
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct CashAccount {
    /// Plain account number. The `{accountNumber}` URL path segment uses the
    /// encrypted [`AccountHash`] instead.
    #[serde(rename = "accountNumber")]
    pub account_number: AccountNumber,
    /// Round-trip count used for PDT classification.
    #[serde(rename = "roundTrips", default)]
    pub round_trips: i32,
    /// `true` if Schwab has flagged this account as a pattern day trader.
    #[serde(rename = "isDayTrader", default)]
    pub is_day_trader: bool,
    /// `true` if Schwab has restricted the account to closing-only trades.
    #[serde(rename = "isClosingOnlyRestricted", default)]
    pub is_closing_only_restricted: bool,
    /// Schwab `pfcbFlag` (post free credit balance) indicator.
    #[serde(rename = "pfcbFlag", default)]
    pub pfcb_flag: bool,
    /// Empty unless the request included `fields=positions`.
    #[serde(default)]
    pub positions: Vec<Position>,
    /// Balances at the start of the trading day, before any session activity.
    #[serde(rename = "initialBalances", default)]
    pub initial_balances: Option<CashInitialBalance>,
    /// Balances reflecting all settled activity to date.
    #[serde(rename = "currentBalances", default)]
    pub current_balances: Option<CashBalance>,
    /// Balances Schwab projects after pending (unsettled) activity clears.
    #[serde(rename = "projectedBalances", default)]
    pub projected_balances: Option<CashBalance>,
}

/// Margin-account `initialBalances` block: snapshot at session start.
///
/// Every field is `Option<Decimal>`; `None` distinguishes "Schwab omitted the
/// field" from "Schwab sent zero." All money values are USD.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MarginInitialBalance {
    /// Interest accrued but not yet posted.
    #[serde(default, with = "decimal_opt", rename = "accruedInterest")]
    pub accrued_interest: Option<Decimal>,
    /// Funds available to trade non-marginable securities.
    #[serde(
        default,
        with = "decimal_opt",
        rename = "availableFundsNonMarginableTrade"
    )]
    pub available_funds_non_marginable_trade: Option<Decimal>,
    /// Market value of bond holdings.
    #[serde(default, with = "decimal_opt", rename = "bondValue")]
    pub bond_value: Option<Decimal>,
    /// Total buying power.
    #[serde(default, with = "decimal_opt", rename = "buyingPower")]
    pub buying_power: Option<Decimal>,
    /// Cash on hand (settled + unsettled).
    #[serde(default, with = "decimal_opt", rename = "cashBalance")]
    pub cash_balance: Option<Decimal>,
    /// Cash available for new trades after pending activity.
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForTrading")]
    pub cash_available_for_trading: Option<Decimal>,
    /// Pending cash receipts not yet settled.
    #[serde(default, with = "decimal_opt", rename = "cashReceipts")]
    pub cash_receipts: Option<Decimal>,
    /// Day-trading buying power (4x rule for PDT accounts).
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPower")]
    pub day_trading_buying_power: Option<Decimal>,
    /// Outstanding day-trading buying-power call, if any.
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPowerCall")]
    pub day_trading_buying_power_call: Option<Decimal>,
    /// Outstanding day-trading equity call, if any.
    #[serde(default, with = "decimal_opt", rename = "dayTradingEquityCall")]
    pub day_trading_equity_call: Option<Decimal>,
    /// Account equity (assets minus margin debt).
    #[serde(default, with = "decimal_opt")]
    pub equity: Option<Decimal>,
    /// Equity as a percentage of total account value.
    #[serde(default, with = "decimal_opt", rename = "equityPercentage")]
    pub equity_percentage: Option<Decimal>,
    /// Liquidation value if every position were closed at the mark.
    #[serde(default, with = "decimal_opt", rename = "liquidationValue")]
    pub liquidation_value: Option<Decimal>,
    /// Long margin value (loanable portion of long positions).
    #[serde(default, with = "decimal_opt", rename = "longMarginValue")]
    pub long_margin_value: Option<Decimal>,
    /// Market value of long option positions.
    #[serde(default, with = "decimal_opt", rename = "longOptionMarketValue")]
    pub long_option_market_value: Option<Decimal>,
    /// Market value of long stock positions.
    #[serde(default, with = "decimal_opt", rename = "longStockValue")]
    pub long_stock_value: Option<Decimal>,
    /// Outstanding maintenance call, if any.
    #[serde(default, with = "decimal_opt", rename = "maintenanceCall")]
    pub maintenance_call: Option<Decimal>,
    /// Maintenance margin requirement.
    #[serde(default, with = "decimal_opt", rename = "maintenanceRequirement")]
    pub maintenance_requirement: Option<Decimal>,
    /// Margin loan balance.
    #[serde(default, with = "decimal_opt")]
    pub margin: Option<Decimal>,
    /// Equity in the margin account.
    #[serde(default, with = "decimal_opt", rename = "marginEquity")]
    pub margin_equity: Option<Decimal>,
    /// Money-market fund holdings.
    #[serde(default, with = "decimal_opt", rename = "moneyMarketFund")]
    pub money_market_fund: Option<Decimal>,
    /// Market value of mutual-fund holdings.
    #[serde(default, with = "decimal_opt", rename = "mutualFundValue")]
    pub mutual_fund_value: Option<Decimal>,
    /// Outstanding Reg-T call, if any.
    #[serde(default, with = "decimal_opt", rename = "regTCall")]
    pub reg_t_call: Option<Decimal>,
    /// Short margin value.
    #[serde(default, with = "decimal_opt", rename = "shortMarginValue")]
    pub short_margin_value: Option<Decimal>,
    /// Market value of short option positions.
    #[serde(default, with = "decimal_opt", rename = "shortOptionMarketValue")]
    pub short_option_market_value: Option<Decimal>,
    /// Market value of short stock positions.
    #[serde(default, with = "decimal_opt", rename = "shortStockValue")]
    pub short_stock_value: Option<Decimal>,
    /// Total cash across settlement classes.
    #[serde(default, with = "decimal_opt", rename = "totalCash")]
    pub total_cash: Option<Decimal>,
    /// `true` if the account is in a call.
    // Schwab's spec types this as a number, but the live API sends a boolean.
    #[serde(default, rename = "isInCall")]
    pub is_in_call: Option<bool>,
    /// Cash from pending trades not yet settled.
    #[serde(default, with = "decimal_opt", rename = "unsettledCash")]
    pub unsettled_cash: Option<Decimal>,
    /// Deposits in transit, not yet available to trade.
    #[serde(default, with = "decimal_opt", rename = "pendingDeposits")]
    pub pending_deposits: Option<Decimal>,
    /// Net margin balance (debit if borrowed, credit if not).
    #[serde(default, with = "decimal_opt", rename = "marginBalance")]
    pub margin_balance: Option<Decimal>,
    /// Net short balance.
    #[serde(default, with = "decimal_opt", rename = "shortBalance")]
    pub short_balance: Option<Decimal>,
    /// Total account value.
    #[serde(default, with = "decimal_opt", rename = "accountValue")]
    pub account_value: Option<Decimal>,
}

/// Margin-account current / projected balances. Same units as
/// [`MarginInitialBalance`] (USD, `None` means Schwab omitted the field).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MarginBalance {
    /// Funds available to enter new trades.
    #[serde(default, with = "decimal_opt", rename = "availableFunds")]
    pub available_funds: Option<Decimal>,
    /// Funds available to trade non-marginable securities.
    #[serde(
        default,
        with = "decimal_opt",
        rename = "availableFundsNonMarginableTrade"
    )]
    pub available_funds_non_marginable_trade: Option<Decimal>,
    /// Total buying power.
    #[serde(default, with = "decimal_opt", rename = "buyingPower")]
    pub buying_power: Option<Decimal>,
    /// Buying power for non-marginable securities.
    #[serde(
        default,
        with = "decimal_opt",
        rename = "buyingPowerNonMarginableTrade"
    )]
    pub buying_power_non_marginable_trade: Option<Decimal>,
    /// Day-trading buying power.
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPower")]
    pub day_trading_buying_power: Option<Decimal>,
    /// Outstanding day-trading buying-power call.
    #[serde(default, with = "decimal_opt", rename = "dayTradingBuyingPowerCall")]
    pub day_trading_buying_power_call: Option<Decimal>,
    /// Account equity.
    #[serde(default, with = "decimal_opt")]
    pub equity: Option<Decimal>,
    /// Equity as a percentage of total account value.
    #[serde(default, with = "decimal_opt", rename = "equityPercentage")]
    pub equity_percentage: Option<Decimal>,
    /// Long margin value.
    #[serde(default, with = "decimal_opt", rename = "longMarginValue")]
    pub long_margin_value: Option<Decimal>,
    /// Outstanding maintenance call.
    #[serde(default, with = "decimal_opt", rename = "maintenanceCall")]
    pub maintenance_call: Option<Decimal>,
    /// Maintenance margin requirement.
    #[serde(default, with = "decimal_opt", rename = "maintenanceRequirement")]
    pub maintenance_requirement: Option<Decimal>,
    /// Net margin balance.
    #[serde(default, with = "decimal_opt", rename = "marginBalance")]
    pub margin_balance: Option<Decimal>,
    /// Outstanding Reg-T call.
    #[serde(default, with = "decimal_opt", rename = "regTCall")]
    pub reg_t_call: Option<Decimal>,
    /// Net short balance.
    #[serde(default, with = "decimal_opt", rename = "shortBalance")]
    pub short_balance: Option<Decimal>,
    /// Short margin value.
    #[serde(default, with = "decimal_opt", rename = "shortMarginValue")]
    pub short_margin_value: Option<Decimal>,
    /// Special memorandum account balance.
    #[serde(default, with = "decimal_opt")]
    pub sma: Option<Decimal>,
    /// `true` if the account is in a call.
    // Schwab's spec types this as a number, but the live API sends a boolean.
    #[serde(default, rename = "isInCall")]
    pub is_in_call: Option<bool>,
    /// Buying power available specifically for stock trades.
    #[serde(default, with = "decimal_opt", rename = "stockBuyingPower")]
    pub stock_buying_power: Option<Decimal>,
    /// Buying power available specifically for option trades.
    #[serde(default, with = "decimal_opt", rename = "optionBuyingPower")]
    pub option_buying_power: Option<Decimal>,
}

/// Cash-account `initialBalances` block: snapshot at session start.
///
/// Every field is `Option<Decimal>`; `None` means Schwab omitted the field.
/// USD throughout.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CashInitialBalance {
    /// Interest accrued but not yet posted.
    #[serde(default, with = "decimal_opt", rename = "accruedInterest")]
    pub accrued_interest: Option<Decimal>,
    /// Cash available for new trades.
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForTrading")]
    pub cash_available_for_trading: Option<Decimal>,
    /// Cash available for withdrawal.
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForWithdrawal")]
    pub cash_available_for_withdrawal: Option<Decimal>,
    /// Cash on hand (settled + unsettled).
    #[serde(default, with = "decimal_opt", rename = "cashBalance")]
    pub cash_balance: Option<Decimal>,
    /// Market value of bond holdings.
    #[serde(default, with = "decimal_opt", rename = "bondValue")]
    pub bond_value: Option<Decimal>,
    /// Pending cash receipts not yet settled.
    #[serde(default, with = "decimal_opt", rename = "cashReceipts")]
    pub cash_receipts: Option<Decimal>,
    /// Liquidation value if every position were closed at the mark.
    #[serde(default, with = "decimal_opt", rename = "liquidationValue")]
    pub liquidation_value: Option<Decimal>,
    /// Market value of long option positions.
    #[serde(default, with = "decimal_opt", rename = "longOptionMarketValue")]
    pub long_option_market_value: Option<Decimal>,
    /// Market value of long stock positions.
    #[serde(default, with = "decimal_opt", rename = "longStockValue")]
    pub long_stock_value: Option<Decimal>,
    /// Money-market fund holdings.
    #[serde(default, with = "decimal_opt", rename = "moneyMarketFund")]
    pub money_market_fund: Option<Decimal>,
    /// Market value of mutual-fund holdings.
    #[serde(default, with = "decimal_opt", rename = "mutualFundValue")]
    pub mutual_fund_value: Option<Decimal>,
    /// Market value of short option positions.
    #[serde(default, with = "decimal_opt", rename = "shortOptionMarketValue")]
    pub short_option_market_value: Option<Decimal>,
    /// Market value of short stock positions.
    #[serde(default, with = "decimal_opt", rename = "shortStockValue")]
    pub short_stock_value: Option<Decimal>,
    /// `true` if the account is in a call.
    // Schwab's spec types this as a number, but the live API sends a boolean.
    #[serde(default, rename = "isInCall")]
    pub is_in_call: Option<bool>,
    /// Cash from pending trades not yet settled.
    #[serde(default, with = "decimal_opt", rename = "unsettledCash")]
    pub unsettled_cash: Option<Decimal>,
    /// Outstanding cash-debit call, if any.
    #[serde(default, with = "decimal_opt", rename = "cashDebitCallValue")]
    pub cash_debit_call_value: Option<Decimal>,
    /// Deposits in transit, not yet available to trade.
    #[serde(default, with = "decimal_opt", rename = "pendingDeposits")]
    pub pending_deposits: Option<Decimal>,
    /// Total account value.
    #[serde(default, with = "decimal_opt", rename = "accountValue")]
    pub account_value: Option<Decimal>,
}

/// Cash-account current / projected balances. Same units as
/// [`CashInitialBalance`].
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CashBalance {
    /// Cash available for new trades.
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForTrading")]
    pub cash_available_for_trading: Option<Decimal>,
    /// Cash available for withdrawal.
    #[serde(default, with = "decimal_opt", rename = "cashAvailableForWithdrawal")]
    pub cash_available_for_withdrawal: Option<Decimal>,
    /// Outstanding cash call.
    #[serde(default, with = "decimal_opt", rename = "cashCall")]
    pub cash_call: Option<Decimal>,
    /// Market value of long non-marginable positions.
    #[serde(default, with = "decimal_opt", rename = "longNonMarginableMarketValue")]
    pub long_non_marginable_market_value: Option<Decimal>,
    /// Total cash across settlement classes.
    #[serde(default, with = "decimal_opt", rename = "totalCash")]
    pub total_cash: Option<Decimal>,
    /// Outstanding cash-debit call.
    #[serde(default, with = "decimal_opt", rename = "cashDebitCallValue")]
    pub cash_debit_call_value: Option<Decimal>,
    /// Cash from pending trades not yet settled.
    #[serde(default, with = "decimal_opt", rename = "unsettledCash")]
    pub unsettled_cash: Option<Decimal>,
}

/// One open position in an account.
///
/// Schwab reports both long and short sides on the same row. Quantities are
/// signed by side (`long_quantity` for the long leg, `short_quantity` for the
/// short leg). All monetary values are USD.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Position {
    /// Short quantity (shares / contracts).
    #[serde(default, with = "decimal_opt", rename = "shortQuantity")]
    pub short_quantity: Option<Decimal>,
    /// Average open price across all lots on this position.
    #[serde(default, with = "decimal_opt", rename = "averagePrice")]
    pub average_price: Option<Decimal>,
    /// P/L accrued during the current trading session.
    #[serde(default, with = "decimal_opt", rename = "currentDayProfitLoss")]
    pub current_day_profit_loss: Option<Decimal>,
    /// Current-session P/L as a percentage of opening basis.
    #[serde(
        default,
        with = "decimal_opt",
        rename = "currentDayProfitLossPercentage"
    )]
    pub current_day_profit_loss_percentage: Option<Decimal>,
    /// Long quantity (shares / contracts).
    #[serde(default, with = "decimal_opt", rename = "longQuantity")]
    pub long_quantity: Option<Decimal>,
    /// Long quantity that has settled.
    #[serde(default, with = "decimal_opt", rename = "settledLongQuantity")]
    pub settled_long_quantity: Option<Decimal>,
    /// Short quantity that has settled.
    #[serde(default, with = "decimal_opt", rename = "settledShortQuantity")]
    pub settled_short_quantity: Option<Decimal>,
    /// Quantity past its aging threshold (used in some buying-power rules).
    #[serde(default, with = "decimal_opt", rename = "agedQuantity")]
    pub aged_quantity: Option<Decimal>,
    /// Instrument identifying what the position is in.
    #[serde(default)]
    pub instrument: Option<AccountsInstrument>,
    /// Mark-to-market value of the position.
    #[serde(default, with = "decimal_opt", rename = "marketValue")]
    pub market_value: Option<Decimal>,
    /// Maintenance margin requirement for this position.
    #[serde(default, with = "decimal_opt", rename = "maintenanceRequirement")]
    pub maintenance_requirement: Option<Decimal>,
    /// Average open price of the long leg.
    #[serde(default, with = "decimal_opt", rename = "averageLongPrice")]
    pub average_long_price: Option<Decimal>,
    /// Average open price of the short leg.
    #[serde(default, with = "decimal_opt", rename = "averageShortPrice")]
    pub average_short_price: Option<Decimal>,
    /// Tax-lot weighted average price of the long leg.
    #[serde(default, with = "decimal_opt", rename = "taxLotAverageLongPrice")]
    pub tax_lot_average_long_price: Option<Decimal>,
    /// Tax-lot weighted average price of the short leg.
    #[serde(default, with = "decimal_opt", rename = "taxLotAverageShortPrice")]
    pub tax_lot_average_short_price: Option<Decimal>,
    /// Open P/L on the long leg.
    #[serde(default, with = "decimal_opt", rename = "longOpenProfitLoss")]
    pub long_open_profit_loss: Option<Decimal>,
    /// Open P/L on the short leg.
    #[serde(default, with = "decimal_opt", rename = "shortOpenProfitLoss")]
    pub short_open_profit_loss: Option<Decimal>,
    /// Long quantity carried over from the prior session.
    #[serde(default, with = "decimal_opt", rename = "previousSessionLongQuantity")]
    pub previous_session_long_quantity: Option<Decimal>,
    /// Short quantity carried over from the prior session.
    #[serde(default, with = "decimal_opt", rename = "previousSessionShortQuantity")]
    pub previous_session_short_quantity: Option<Decimal>,
    /// Net cost basis of trades executed in the current session.
    #[serde(default, with = "decimal_opt", rename = "currentDayCost")]
    pub current_day_cost: Option<Decimal>,
}

/// Instrument carried by a `Position`. Flat struct: every field that exists
/// on any documented asset variant lives here as `Option`, so newly-added
/// asset types deserialize cleanly even if this crate has not been updated.
/// Consumers match on [`AccountsInstrument::asset_type`] to route.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct AccountsInstrument {
    /// Asset-class discriminator. Match on this to interpret the variant-
    /// specific fields below.
    #[serde(rename = "assetType")]
    pub asset_type: AssetType,
    /// CUSIP, when Schwab has assigned one.
    #[serde(default)]
    pub cusip: Option<String>,
    /// Wire symbol (Schwab format - e.g. OCC OSI for options).
    #[serde(default)]
    pub symbol: Option<String>,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Schwab-internal instrument id.
    #[serde(default, rename = "instrumentId")]
    pub instrument_id: Option<i64>,
    /// Net price change since the prior close, USD.
    #[serde(default, with = "decimal_opt", rename = "netChange")]
    pub net_change: Option<Decimal>,

    // Option-specific fields. `None` on non-option asset types.
    /// Option deliverables. Empty on non-option asset types.
    #[serde(default, rename = "optionDeliverables")]
    pub option_deliverables: Vec<AccountApiOptionDeliverable>,
    /// `Put` / `Call` flag for options.
    #[serde(default, rename = "putCall")]
    pub put_call: Option<PutCall>,
    /// Shares-per-contract multiplier for options (typically 100).
    #[serde(default, rename = "optionMultiplier")]
    pub option_multiplier: Option<i32>,
    /// Option style (`VANILLA` / `BINARY` / `BARRIER`).
    #[serde(default, rename = "type")]
    pub option_type: Option<OptionType>,
    /// Symbol of the underlying instrument for options.
    #[serde(default, rename = "underlyingSymbol")]
    pub underlying_symbol: Option<String>,

    // Fixed-income-specific fields. `None` on non-fixed-income asset types.
    /// Maturity date for fixed-income instruments.
    #[serde(default, rename = "maturityDate")]
    pub maturity_date: Option<DateTime<Utc>>,
    /// Mortgage-backed pool factor (remaining principal fraction).
    #[serde(default, with = "decimal_opt")]
    pub factor: Option<Decimal>,
    /// Current coupon rate for floating-rate fixed-income instruments.
    #[serde(default, with = "decimal_opt", rename = "variableRate")]
    pub variable_rate: Option<Decimal>,
}

/// One deliverable component of an option contract (the security delivered
/// per contract on assignment / exercise).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct AccountApiOptionDeliverable {
    /// Symbol of the deliverable security.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Units delivered per contract.
    #[serde(default, with = "decimal_opt", rename = "deliverableUnits")]
    pub deliverable_units: Option<Decimal>,
    /// Settlement currency.
    #[serde(default, rename = "apiCurrencyType")]
    pub currency_type: Option<ApiCurrencyType>,
    /// Asset class of the deliverable.
    #[serde(default, rename = "assetType")]
    pub asset_type: Option<AssetType>,
}

string_enum! {
    /// Schwab `assetType` discriminator.
    AssetType {
        /// Listed equity (common / preferred / ADR).
        Equity = "EQUITY",
        /// Mutual fund.
        MutualFund = "MUTUAL_FUND",
        /// Listed option contract.
        Option = "OPTION",
        /// Futures contract.
        Future = "FUTURE",
        /// Foreign-exchange pair.
        Forex = "FOREX",
        /// Market index (non-tradeable reference).
        Index = "INDEX",
        /// Money-market or other cash-equivalent.
        CashEquivalent = "CASH_EQUIVALENT",
        /// Fixed-income security (bond, note, bill).
        FixedIncome = "FIXED_INCOME",
        /// Schwab "product" wrapper (structured product, etc.).
        Product = "PRODUCT",
        /// Currency (cash holding).
        Currency = "CURRENCY",
        /// Collective investment trust or similar pooled vehicle.
        CollectiveInvestment = "COLLECTIVE_INVESTMENT",
    }
}

impl Default for AssetType {
    fn default() -> Self {
        AssetType::Unknown(String::new())
    }
}

string_enum! {
    /// Whether an option contract is a put or a call.
    PutCall {
        /// Put.
        Put = "PUT",
        /// Call.
        Call = "CALL",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Option payoff style.
    OptionType {
        /// Standard listed option.
        Vanilla = "VANILLA",
        /// Binary (cash-or-nothing) option.
        Binary = "BINARY",
        /// Barrier option.
        Barrier = "BARRIER",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Settlement currency for an option deliverable.
    ApiCurrencyType {
        /// US dollar.
        Usd = "USD",
        /// Canadian dollar.
        Cad = "CAD",
        /// Euro.
        Eur = "EUR",
        /// Japanese yen.
        Jpy = "JPY",
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
    fn unknown_account_type_parses_into_unknown_variant() {
        // A `type` Schwab might add later (e.g., a custodial / retirement
        // account type) must not fail the whole response - it lands in
        // SecuritiesAccount::Unknown with the raw object preserved.
        let json = r#"{
            "securitiesAccount": {
                "type": "FUTURES",
                "accountNumber": "99999999",
                "extraField": 42
            }
        }"#;
        let parsed: Account = serde_json::from_str(json).unwrap();
        match &parsed.securities_account {
            SecuritiesAccount::Unknown { account_type, raw } => {
                assert_eq!(account_type, "FUTURES");
                assert_eq!(
                    raw.get("accountNumber").and_then(|v| v.as_str()),
                    Some("99999999")
                );
                assert_eq!(raw.get("extraField").and_then(|v| v.as_i64()), Some(42));
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
        // Accessors return safe defaults / None for unknown account types.
        assert_eq!(parsed.securities_account.account_type(), "FUTURES");
        assert!(parsed.securities_account.account_number().is_none());
        assert!(parsed.securities_account.positions().is_empty());
        assert!(parsed.securities_account.is_day_trader().is_none());
    }

    #[test]
    fn missing_account_type_is_a_parse_error() {
        // Schwab's spec requires `type`; a payload missing it is malformed
        // (not a forward-compat case) and must fail loudly.
        let json = r#"{
            "securitiesAccount": {
                "accountNumber": "99999999"
            }
        }"#;
        let err = serde_json::from_str::<Account>(json).unwrap_err();
        assert!(err.to_string().contains("type"), "got: {err}");
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
