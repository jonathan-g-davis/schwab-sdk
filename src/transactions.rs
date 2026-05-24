//! `GET /accounts/{accountNumber}/transactions*` - Schwab Trader API.
//!
//! Endpoints:
//!
//! - `GET /accounts/{accountNumber}/transactions` lists transactions for an
//!   account, filtered by required start/end dates and transaction `type`,
//!   plus an optional `symbol`. Maximum 3000 results per call and a 1-year
//!   date range per Schwab's documentation.
//! - `GET /accounts/{accountNumber}/transactions/{transactionId}` returns a
//!   single transaction by ID. The OpenAPI spec types the response as an
//!   array; this crate matches the spec.
//!
//! `{accountNumber}` is the encrypted [`AccountHash`], not the plain
//! account number.
//!
//! Reached through
//! [`SchwabClient::transactions`](crate::SchwabClient::transactions).

use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use crate::client::SchwabClient;
use crate::error::Result;
use crate::macros::string_enum;
use crate::secrets::{AccountHash, AccountNumber};

/// Accessor for the `/accounts/{accountNumber}/transactions*` endpoint
/// family. Construct via [`SchwabClient::transactions`].
pub struct Transactions<'a, 'b> {
    client: &'a SchwabClient,
    account_hash: &'b AccountHash,
}

impl<'a, 'b> Transactions<'a, 'b> {
    pub(crate) fn new(client: &'a SchwabClient, account_hash: &'b AccountHash) -> Self {
        Self {
            client,
            account_hash,
        }
    }

    /// Begin a `GET /accounts/{accountNumber}/transactions` request.
    ///
    /// All three parameters are required by Schwab:
    /// - `start_date` and `end_date` bound the result window. Schwab caps
    ///   the window at one year; this builder does not enforce that.
    /// - `types` filters to a single [`TransactionType`].
    ///
    /// Optional filters (e.g. [`ListTransactionsBuilder::symbol`]) chain
    /// before [`ListTransactionsBuilder::send`].
    pub fn list(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        types: TransactionType,
    ) -> ListTransactionsBuilder<'a, 'b> {
        ListTransactionsBuilder {
            client: self.client,
            account_hash: self.account_hash,
            start_date,
            end_date,
            types,
            symbol: None,
        }
    }

    /// `GET /accounts/{accountNumber}/transactions/{transactionId}` - fetch
    /// a single transaction. Schwab returns it wrapped in a one-element
    /// array per their OpenAPI spec.
    pub async fn get(&self, transaction_id: i64) -> Result<Vec<Transaction>> {
        let hash = self.account_hash.expose_secret();
        let path = format!("/accounts/{hash}/transactions/{transaction_id}");
        self.client.trader_http().get_json(&path).await
    }
}

/// In-flight request for `GET /accounts/{accountNumber}/transactions`.
/// Built via [`Transactions::list`].
#[must_use = "call .send() to execute the request"]
pub struct ListTransactionsBuilder<'a, 'b> {
    client: &'a SchwabClient,
    account_hash: &'b AccountHash,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    types: TransactionType,
    symbol: Option<String>,
}

impl<'a, 'b> ListTransactionsBuilder<'a, 'b> {
    /// Restrict the response to transactions touching a single symbol.
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    pub async fn send(self) -> Result<Vec<Transaction>> {
        let hash = self.account_hash.expose_secret();
        // Schwab's documented format is `yyyy-MM-dd'T'HH:mm:ss.SSSZ`;
        // `to_rfc3339_opts(Millis, true)` yields exactly that shape.
        let start = self.start_date.to_rfc3339_opts(SecondsFormat::Millis, true);
        let end = self.end_date.to_rfc3339_opts(SecondsFormat::Millis, true);
        let types = self.types.to_string();

        let trader = self.client.trader_http();
        let mut request = trader
            .get(&format!("/accounts/{hash}/transactions"))
            .query(&[
                ("startDate", start.as_str()),
                ("endDate", end.as_str()),
                ("types", types.as_str()),
            ]);
        if let Some(sym) = &self.symbol {
            request = request.query(&[("symbol", sym.as_str())]);
        }
        trader.execute_json(request).await
    }
}

/// One transaction record. The `activity_type` field discriminates
/// what kind of activity this row represents.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Transaction {
    #[serde(default, rename = "activityId")]
    pub activity_id: Option<i64>,
    /// Time the transaction was recorded.
    #[serde(default)]
    pub time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub user: Option<UserDetails>,
    #[serde(default)]
    pub description: Option<String>,
    /// Plain account number that owns this transaction.
    #[serde(default, rename = "accountNumber")]
    pub account_number: Option<AccountNumber>,
    #[serde(default, rename = "type")]
    pub transaction_type: Option<TransactionType>,
    #[serde(default)]
    pub status: Option<TransactionStatus>,
    #[serde(default, rename = "subAccount")]
    pub sub_account: Option<SubAccount>,
    #[serde(default, rename = "tradeDate")]
    pub trade_date: Option<DateTime<Utc>>,
    #[serde(default, rename = "settlementDate")]
    pub settlement_date: Option<DateTime<Utc>>,
    #[serde(default, rename = "positionId")]
    pub position_id: Option<i64>,
    #[serde(default, rename = "orderId")]
    pub order_id: Option<i64>,
    #[serde(default, with = "decimal_opt", rename = "netAmount")]
    pub net_amount: Option<Decimal>,
    #[serde(default, rename = "activityType")]
    pub activity_type: Option<ActivityType>,
    #[serde(default, rename = "transferItems")]
    pub transfer_items: Vec<TransferItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct UserDetails {
    #[serde(default, rename = "cdDomainId")]
    pub cd_domain_id: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default, rename = "type")]
    pub user_type: Option<UserType>,
    #[serde(default, rename = "userId")]
    pub user_id: Option<i64>,
    #[serde(default, rename = "systemUserName")]
    pub system_user_name: Option<String>,
    #[serde(default, rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(default, rename = "lastName")]
    pub last_name: Option<String>,
    #[serde(default, rename = "brokerRepCode")]
    pub broker_rep_code: Option<String>,
}

/// One leg of a transaction. A trade typically has a security TransferItem
/// (the instrument moved) and one or more fee TransferItems (commission,
/// SEC fee, etc.) distinguished by `fee_type`.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct TransferItem {
    #[serde(default)]
    pub instrument: Option<TransactionInstrument>,
    #[serde(default, with = "decimal_opt")]
    pub amount: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub cost: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub price: Option<Decimal>,
    #[serde(default, rename = "feeType")]
    pub fee_type: Option<FeeType>,
    #[serde(default, rename = "positionEffect")]
    pub position_effect: Option<PositionEffect>,
}

/// Instrument referenced by a `TransferItem`. Flat struct: every documented
/// field across the eleven asset-type variants is here as `Option`, so
/// newly added asset types or fields deserialize cleanly even if this crate
/// has not been updated. Consumers match on [`TransactionInstrument::asset_type`]
/// to route.
///
/// The `type` discriminator inside a variant (e.g. `COMMON_STOCK` for an
/// equity, `VANILLA` for an option, `US_TREASURY_BOND` for fixed income) is
/// preserved as a raw string in [`Self::variant_type`].
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct TransactionInstrument {
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
    /// Variant-specific subtype string (e.g. `COMMON_STOCK`, `VANILLA`,
    /// `MONEY_MARKET_FUND`). Kept as a raw string because the value space
    /// differs per asset type.
    #[serde(default, rename = "type")]
    pub variant_type: Option<String>,

    // Option fields. `None` for non-futures/options.
    #[serde(default, rename = "expirationDate")]
    pub expiration_date: Option<DateTime<Utc>>,
    #[serde(default, rename = "optionDeliverables")]
    pub option_deliverables: Vec<TransactionApiOptionDeliverable>,
    #[serde(default, rename = "optionPremiumMultiplier")]
    pub option_premium_multiplier: Option<i64>,
    #[serde(default, rename = "putCall")]
    pub put_call: Option<PutCall>,
    #[serde(default, with = "decimal_opt", rename = "strikePrice")]
    pub strike_price: Option<Decimal>,
    #[serde(default, rename = "underlyingSymbol")]
    pub underlying_symbol: Option<String>,
    #[serde(default, rename = "underlyingCusip")]
    pub underlying_cusip: Option<String>,
    /// `TransactionOption.deliverable`: the instrument delivered when an
    /// option is exercised or assigned. Boxed because
    /// `TransactionInstrument` references itself through this field.
    #[serde(default)]
    pub deliverable: Option<Box<TransactionInstrument>>,

    // Fixed-income fields.
    #[serde(default, rename = "maturityDate")]
    pub maturity_date: Option<DateTime<Utc>>,
    #[serde(default, with = "decimal_opt")]
    pub factor: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub multiplier: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "variableRate")]
    pub variable_rate: Option<Decimal>,

    // Mutual-fund fields.
    #[serde(default, rename = "fundFamilyName")]
    pub fund_family_name: Option<String>,
    #[serde(default, rename = "fundFamilySymbol")]
    pub fund_family_symbol: Option<String>,
    #[serde(default, rename = "fundGroup")]
    pub fund_group: Option<String>,
    #[serde(default, rename = "exchangeCutoffTime")]
    pub exchange_cutoff_time: Option<DateTime<Utc>>,
    #[serde(default, rename = "purchaseCutoffTime")]
    pub purchase_cutoff_time: Option<DateTime<Utc>>,
    #[serde(default, rename = "redemptionCutoffTime")]
    pub redemption_cutoff_time: Option<DateTime<Utc>>,

    // Future / index fields.
    //
    // `Future.expirationDate` shares the `expiration_date` field defined
    // above (the spec uses the same wire name on both `Future` and
    // `TransactionOption`).
    #[serde(default, rename = "activeContract")]
    pub active_contract: Option<bool>,
    #[serde(default, rename = "lastTradingDate")]
    pub last_trading_date: Option<DateTime<Utc>>,
    #[serde(default, rename = "firstNoticeDate")]
    pub first_notice_date: Option<DateTime<Utc>>,

    // Forex fields. Each currency is itself a `TransactionInstrument`
    // with `assetType` = `CURRENCY`; boxed to break the recursive type.
    #[serde(default, rename = "baseCurrency")]
    pub base_currency: Option<Box<TransactionInstrument>>,
    #[serde(default, rename = "counterCurrency")]
    pub counter_currency: Option<Box<TransactionInstrument>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct TransactionApiOptionDeliverable {
    #[serde(default, rename = "rootSymbol")]
    pub root_symbol: Option<String>,
    #[serde(default, rename = "strikePercent")]
    pub strike_percent: Option<i64>,
    #[serde(default, rename = "deliverableNumber")]
    pub deliverable_number: Option<i64>,
    #[serde(default, with = "decimal_opt", rename = "deliverableUnits")]
    pub deliverable_units: Option<Decimal>,
    /// The instrument that will be delivered for this deliverable entry
    /// (e.g. shares of the surviving issuer after a corporate action).
    /// Boxed because `TransactionInstrument` references itself through
    /// this field.
    #[serde(default)]
    pub deliverable: Option<Box<TransactionInstrument>>,
    #[serde(default, rename = "assetType")]
    pub asset_type: Option<AssetType>,
}

// --- Enums ---

string_enum! {
    /// `types` query parameter for [`Transactions::list`] and the `type`
    /// field on a [`Transaction`].
    TransactionType {
        Trade = "TRADE",
        ReceiveAndDeliver = "RECEIVE_AND_DELIVER",
        DividendOrInterest = "DIVIDEND_OR_INTEREST",
        AchReceipt = "ACH_RECEIPT",
        AchDisbursement = "ACH_DISBURSEMENT",
        CashReceipt = "CASH_RECEIPT",
        CashDisbursement = "CASH_DISBURSEMENT",
        ElectronicFund = "ELECTRONIC_FUND",
        WireOut = "WIRE_OUT",
        WireIn = "WIRE_IN",
        Journal = "JOURNAL",
        Memorandum = "MEMORANDUM",
        MarginCall = "MARGIN_CALL",
        MoneyMarket = "MONEY_MARKET",
        SmaAdjustment = "SMA_ADJUSTMENT",
    }
}

string_enum! {
    ActivityType {
        ActivityCorrection = "ACTIVITY_CORRECTION",
        Execution = "EXECUTION",
        OrderAction = "ORDER_ACTION",
        Transfer = "TRANSFER",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    TransactionStatus {
        Valid = "VALID",
        Invalid = "INVALID",
        Pending = "PENDING",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    SubAccount {
        Cash = "CASH",
        Margin = "MARGIN",
        Short = "SHORT",
        Div = "DIV",
        Income = "INCOME",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    UserType {
        Advisor = "ADVISOR_USER",
        Broker = "BROKER_USER",
        Client = "CLIENT_USER",
        System = "SYSTEM_USER",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Variant order matches the order of the `FeeType` enum in the
    /// Schwab Trader API OpenAPI spec.
    FeeType {
        Commission = "COMMISSION",
        SecFee = "SEC_FEE",
        StrFee = "STR_FEE",
        RFee = "R_FEE",
        CdscFee = "CDSC_FEE",
        OptRegFee = "OPT_REG_FEE",
        AdditionalFee = "ADDITIONAL_FEE",
        MiscellaneousFee = "MISCELLANEOUS_FEE",
        Ftt = "FTT",
        FuturesClearingFee = "FUTURES_CLEARING_FEE",
        FuturesDeskOfficeFee = "FUTURES_DESK_OFFICE_FEE",
        FuturesExchangeFee = "FUTURES_EXCHANGE_FEE",
        FuturesGlobexFee = "FUTURES_GLOBEX_FEE",
        FuturesNfaFee = "FUTURES_NFA_FEE",
        FuturesPitBrokerageFee = "FUTURES_PIT_BROKERAGE_FEE",
        FuturesTransactionFee = "FUTURES_TRANSACTION_FEE",
        LowProceedsCommission = "LOW_PROCEEDS_COMMISSION",
        BaseCharge = "BASE_CHARGE",
        GeneralCharge = "GENERAL_CHARGE",
        GstFee = "GST_FEE",
        TafFee = "TAF_FEE",
        IndexOptionFee = "INDEX_OPTION_FEE",
        TefraTax = "TEFRA_TAX",
        StateTax = "STATE_TAX",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    PositionEffect {
        Opening = "OPENING",
        Closing = "CLOSING",
        Automatic = "AUTOMATIC",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Asset-type discriminator for [`TransactionInstrument`]. The
    /// transaction schema permits more variants than account positions
    /// (e.g. `FUTURE`, `FOREX`), so this is a distinct enum from
    /// [`crate::accounts::AssetType`]; both share the same
    /// wire-string space and forward-compat catch-all.
    AssetType {
        Equity = "EQUITY",
        Option = "OPTION",
        Index = "INDEX",
        MutualFund = "MUTUAL_FUND",
        CashEquivalent = "CASH_EQUIVALENT",
        FixedIncome = "FIXED_INCOME",
        Currency = "CURRENCY",
        CollectiveInvestment = "COLLECTIVE_INVESTMENT",
        Forex = "FOREX",
        Future = "FUTURE",
        Product = "PRODUCT",
    }
}

impl Default for AssetType {
    fn default() -> Self {
        AssetType::Unknown(String::new())
    }
}

string_enum! {
    PutCall {
        Put = "PUT",
        Call = "CALL",
        UnknownSchwab = "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn trade_with_equity_instrument_parses() {
        let json = r#"{
            "activityId": 9876543210,
            "time": "2024-03-15T15:30:00.000Z",
            "description": "BOUGHT 10 AAPL @ 145.32",
            "accountNumber": "12345678",
            "type": "TRADE",
            "status": "VALID",
            "subAccount": "MARGIN",
            "tradeDate": "2024-03-15T15:30:00.000Z",
            "settlementDate": "2024-03-17T00:00:00.000Z",
            "positionId": 12345,
            "orderId": 67890,
            "netAmount": -1453.20,
            "activityType": "EXECUTION",
            "transferItems": [
                {
                    "instrument": {
                        "assetType": "EQUITY",
                        "symbol": "AAPL",
                        "cusip": "037833100",
                        "description": "Apple Inc",
                        "instrumentId": 12345,
                        "type": "COMMON_STOCK"
                    },
                    "amount": 10,
                    "cost": -1453.20,
                    "price": 145.32,
                    "positionEffect": "OPENING"
                },
                {
                    "amount": -1.00,
                    "feeType": "COMMISSION"
                }
            ]
        }"#;
        let tx: Transaction = serde_json::from_str(json).unwrap();
        assert_eq!(tx.activity_id, Some(9876543210));
        assert_eq!(tx.transaction_type, Some(TransactionType::Trade));
        assert_eq!(tx.activity_type, Some(ActivityType::Execution));
        assert_eq!(tx.status, Some(TransactionStatus::Valid));
        assert_eq!(tx.sub_account, Some(SubAccount::Margin));
        assert_eq!(tx.net_amount, Some(dec!(-1453.20)));
        assert_eq!(tx.transfer_items.len(), 2);

        let security = &tx.transfer_items[0];
        let inst = security.instrument.as_ref().unwrap();
        assert_eq!(inst.asset_type, AssetType::Equity);
        assert_eq!(inst.symbol.as_deref(), Some("AAPL"));
        assert_eq!(inst.variant_type.as_deref(), Some("COMMON_STOCK"));
        assert_eq!(security.amount, Some(dec!(10)));
        assert_eq!(security.price, Some(dec!(145.32)));
        assert_eq!(security.position_effect, Some(PositionEffect::Opening));

        let fee = &tx.transfer_items[1];
        assert_eq!(fee.fee_type, Some(FeeType::Commission));
        assert_eq!(fee.amount, Some(dec!(-1.00)));
        assert!(fee.instrument.is_none());
    }

    #[test]
    fn option_instrument_parses() {
        let json = r#"{
            "assetType": "OPTION",
            "symbol": "AAPL  240315C00200000",
            "underlyingSymbol": "AAPL",
            "underlyingCusip": "037833100",
            "putCall": "CALL",
            "type": "VANILLA",
            "strikePrice": 200.00,
            "expirationDate": "2024-03-15T20:00:00.000Z",
            "optionPremiumMultiplier": 100
        }"#;
        let inst: TransactionInstrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.asset_type, AssetType::Option);
        assert_eq!(inst.put_call, Some(PutCall::Call));
        assert_eq!(inst.variant_type.as_deref(), Some("VANILLA"));
        assert_eq!(inst.strike_price, Some(dec!(200.00)));
        assert_eq!(inst.option_premium_multiplier, Some(100));
        assert_eq!(inst.underlying_symbol.as_deref(), Some("AAPL"));
    }

    #[test]
    fn fixed_income_instrument_parses() {
        let json = r#"{
            "assetType": "FIXED_INCOME",
            "symbol": "912828YK0",
            "description": "US TREASURY NOTE 1.5% 2024",
            "type": "US_TREASURY_NOTE",
            "maturityDate": "2024-08-15T00:00:00.000Z",
            "factor": 1.0,
            "variableRate": 0.015
        }"#;
        let inst: TransactionInstrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.asset_type, AssetType::FixedIncome);
        assert_eq!(inst.variant_type.as_deref(), Some("US_TREASURY_NOTE"));
        assert_eq!(inst.factor, Some(dec!(1.0)));
        assert_eq!(inst.variable_rate, Some(dec!(0.015)));
    }

    #[test]
    fn unknown_transaction_type_preserves_raw_string() {
        let json = r#""SOME_NEW_TXN_KIND""#;
        let parsed: TransactionType = serde_json::from_str(json).unwrap();
        match &parsed {
            TransactionType::Unknown(raw) => assert_eq!(raw, "SOME_NEW_TXN_KIND"),
            other => panic!("expected Unknown, got {other:?}"),
        }
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }

    #[test]
    fn unknown_activity_and_asset_types_preserve_raw_string() {
        let parsed: ActivityType = serde_json::from_str(r#""NEW_ACTIVITY""#).unwrap();
        assert!(matches!(parsed, ActivityType::Unknown(ref s) if s == "NEW_ACTIVITY"));
        let parsed: AssetType = serde_json::from_str(r#""NEW_ASSET""#).unwrap();
        assert!(matches!(parsed, AssetType::Unknown(ref s) if s == "NEW_ASSET"));
    }

    #[test]
    fn transaction_type_round_trips_each_known_variant() {
        for raw in [
            "TRADE",
            "RECEIVE_AND_DELIVER",
            "DIVIDEND_OR_INTEREST",
            "ACH_RECEIPT",
            "ACH_DISBURSEMENT",
            "CASH_RECEIPT",
            "CASH_DISBURSEMENT",
            "ELECTRONIC_FUND",
            "WIRE_OUT",
            "WIRE_IN",
            "JOURNAL",
            "MEMORANDUM",
            "MARGIN_CALL",
            "MONEY_MARKET",
            "SMA_ADJUSTMENT",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: TransactionType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn datetime_fields_parse_iso8601() {
        let json = r#"{
            "tradeDate": "2024-03-15T15:30:00.000Z",
            "settlementDate": "2024-03-17T00:00:00.000Z"
        }"#;
        let tx: Transaction = serde_json::from_str(json).unwrap();
        let trade = tx.trade_date.unwrap();
        assert_eq!(
            trade.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-03-15T15:30:00.000Z"
        );
        assert!(tx.settlement_date.is_some());
    }

    #[test]
    fn fee_type_round_trips_each_known_variant() {
        // Verifies every variant in the spec's `FeeType` enum, including
        // the futures-family fees and tax categories added per the spec
        // audit.
        for raw in [
            "COMMISSION",
            "SEC_FEE",
            "STR_FEE",
            "R_FEE",
            "CDSC_FEE",
            "OPT_REG_FEE",
            "ADDITIONAL_FEE",
            "MISCELLANEOUS_FEE",
            "FTT",
            "FUTURES_CLEARING_FEE",
            "FUTURES_DESK_OFFICE_FEE",
            "FUTURES_EXCHANGE_FEE",
            "FUTURES_GLOBEX_FEE",
            "FUTURES_NFA_FEE",
            "FUTURES_PIT_BROKERAGE_FEE",
            "FUTURES_TRANSACTION_FEE",
            "LOW_PROCEEDS_COMMISSION",
            "BASE_CHARGE",
            "GENERAL_CHARGE",
            "GST_FEE",
            "TAF_FEE",
            "INDEX_OPTION_FEE",
            "TEFRA_TAX",
            "STATE_TAX",
            "UNKNOWN",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: FeeType = serde_json::from_str(&json).unwrap();
            assert!(
                !matches!(parsed, FeeType::Unknown(_)),
                "{raw} fell into the catch-all Unknown variant",
            );
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn option_with_deliverable_parses() {
        // `TransactionOption.deliverable` is set when an option exercises
        // into something other than the plain underlying (e.g. a
        // corporate-action-adjusted equity).
        let json = r#"{
            "assetType": "OPTION",
            "symbol": "AAPL  240315C00200000",
            "underlyingSymbol": "AAPL",
            "putCall": "CALL",
            "type": "VANILLA",
            "strikePrice": 200.00,
            "expirationDate": "2024-03-15T20:00:00.000Z",
            "deliverable": {
                "assetType": "EQUITY",
                "symbol": "AAPL",
                "type": "COMMON_STOCK"
            }
        }"#;
        let inst: TransactionInstrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.asset_type, AssetType::Option);
        let deliverable = inst.deliverable.as_deref().unwrap();
        assert_eq!(deliverable.asset_type, AssetType::Equity);
        assert_eq!(deliverable.symbol.as_deref(), Some("AAPL"));
        assert_eq!(deliverable.variant_type.as_deref(), Some("COMMON_STOCK"));
    }

    #[test]
    fn option_deliverable_entry_with_nested_instrument_parses() {
        // `TransactionAPIOptionDeliverable.deliverable` is the per-entry
        // nested instrument inside `optionDeliverables[]`.
        let json = r#"{
            "assetType": "OPTION",
            "symbol": "XYZ   240620C00050000",
            "underlyingSymbol": "XYZ",
            "putCall": "CALL",
            "type": "VANILLA",
            "optionDeliverables": [
                {
                    "rootSymbol": "XYZ",
                    "strikePercent": 100,
                    "deliverableNumber": 1,
                    "deliverableUnits": 100,
                    "assetType": "EQUITY",
                    "deliverable": {
                        "assetType": "EQUITY",
                        "symbol": "NEWCO",
                        "description": "NewCo Inc post-merger",
                        "type": "COMMON_STOCK"
                    }
                }
            ]
        }"#;
        let inst: TransactionInstrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.option_deliverables.len(), 1);
        let entry = &inst.option_deliverables[0];
        assert_eq!(entry.root_symbol.as_deref(), Some("XYZ"));
        let nested = entry.deliverable.as_deref().unwrap();
        assert_eq!(nested.asset_type, AssetType::Equity);
        assert_eq!(nested.symbol.as_deref(), Some("NEWCO"));
    }

    #[test]
    fn future_with_expiration_date_parses() {
        // Spec defines `expirationDate` on both `TransactionOption` and
        // `Future`.
        let json = r#"{
            "assetType": "FUTURE",
            "symbol": "/ESH24",
            "type": "STANDARD",
            "activeContract": true,
            "expirationDate": "2024-03-15T20:00:00.000Z",
            "lastTradingDate": "2024-03-15T13:30:00.000Z",
            "firstNoticeDate": "2024-03-01T00:00:00.000Z",
            "multiplier": 50
        }"#;
        let inst: TransactionInstrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.asset_type, AssetType::Future);
        assert_eq!(inst.active_contract, Some(true));
        assert!(inst.expiration_date.is_some());
        assert!(inst.last_trading_date.is_some());
        assert!(inst.first_notice_date.is_some());
        assert_eq!(inst.multiplier, Some(dec!(50)));
    }

    #[test]
    fn forex_with_base_and_counter_currency_parses() {
        let json = r#"{
            "assetType": "FOREX",
            "symbol": "EUR/USD",
            "type": "STANDARD",
            "baseCurrency": {
                "assetType": "CURRENCY",
                "symbol": "EUR",
                "description": "Euro"
            },
            "counterCurrency": {
                "assetType": "CURRENCY",
                "symbol": "USD",
                "description": "US Dollar"
            }
        }"#;
        let inst: TransactionInstrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.asset_type, AssetType::Forex);
        let base = inst.base_currency.as_deref().unwrap();
        assert_eq!(base.asset_type, AssetType::Currency);
        assert_eq!(base.symbol.as_deref(), Some("EUR"));
        let counter = inst.counter_currency.as_deref().unwrap();
        assert_eq!(counter.asset_type, AssetType::Currency);
        assert_eq!(counter.symbol.as_deref(), Some("USD"));
    }

    #[test]
    fn datetime_formatting_matches_schwab_wire_format() {
        use chrono::TimeZone;
        let dt = chrono::Utc
            .with_ymd_and_hms(2024, 3, 28, 21, 10, 42)
            .unwrap();
        assert_eq!(
            dt.to_rfc3339_opts(SecondsFormat::Millis, true),
            "2024-03-28T21:10:42.000Z"
        );
    }
}
