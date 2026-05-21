//! `/orders` and `/accounts/{accountNumber}/orders*` - Schwab Trader API.
//!
//! This module currently covers the read endpoints:
//!
//! - `GET /accounts/{accountNumber}/orders` - per-account list with required
//!   `fromEnteredTime` and `toEnteredTime`, optional `maxResults` and
//!   `status`. Schwab caps the date range at 1 year.
//! - `GET /accounts/{accountNumber}/orders/{orderId}` - single fetch.
//! - `GET /orders` - same shape, across every linked account. Date range
//!   is capped at 60 days.
//!
//! `{accountNumber}` is the encrypted [`AccountHash`], not the plain
//! account number. `orderId` is the Schwab-assigned `int64` returned in
//! the `Location` header of a successful place / replace.
//!
//! ## Idempotency
//!
//! Schwab's Trader API exposes **no client-controllable idempotency key**.
//! [`Orders::place`] takes only the order body; if a network or 5xx
//! failure interrupts the response, the order may still have been
//! accepted. Callers that need retry-safe submission must dedupe at
//! their own layer - typically by listing orders after a transient
//! failure and matching by entered-time window, symbol, side, and
//! quantity.

use std::marker::PhantomData;

use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::{Deserialize, Serialize};

use crate::api::accounts::{AccountsInstrument, AssetType};
use crate::error::{Error, Result};
use crate::model::AccountHash;
use crate::rest::SchwabClient;

// --- Namespaces ---

/// Accessor for `/accounts/{accountNumber}/orders*`. Construct via
/// [`SchwabClient::orders`].
pub struct Orders<'a, 'b> {
    client: &'a SchwabClient,
    account_hash: &'b AccountHash,
}

impl<'a, 'b> Orders<'a, 'b> {
    pub(crate) fn new(client: &'a SchwabClient, account_hash: &'b AccountHash) -> Self {
        Self {
            client,
            account_hash,
        }
    }

    /// `GET /accounts/{accountNumber}/orders/{orderId}` - fetch a single
    /// order. `order_id` is the Schwab-assigned `int64` (from the
    /// `Location` header on a place / replace, or from a list call).
    pub async fn get(&self, order_id: i64) -> Result<Order> {
        let hash = self.account_hash.expose_secret();
        let path = format!("/accounts/{hash}/orders/{order_id}");
        self.client.get_json(&path).await
    }

    /// `POST /accounts/{accountNumber}/orders` - place an order.
    ///
    /// On success Schwab returns 201 with an empty body and a `Location`
    /// header pointing at the new order's resource. This method parses the
    /// trailing `{orderId}` segment from that header and returns it.
    /// Callers may then use [`Self::get`] to fetch the placed order's
    /// status and execution detail.
    ///
    /// Schwab has no client-controllable idempotency key, so a transient
    /// failure here may have placed the order anyway. Implementers should
    /// deduplicate orders after a transient failure by listing orders after
    /// a transient failure and matching by entered-time window, symbol,
    /// side, and quantity.
    pub async fn place(&self, order: &OrderRequest) -> Result<i64> {
        let hash = self.account_hash.expose_secret();
        let request = self
            .client
            .post(&format!("/accounts/{hash}/orders"))
            .json(order);
        let response = self.client.execute(request).await?;
        parse_order_id_from_location(&response)
    }

    /// Begin a `GET /accounts/{accountNumber}/orders` request.
    ///
    /// `from_entered_time` and `to_entered_time` bound the result window.
    /// Schwab caps the window at one year; this builder does not enforce
    /// that. Optional filters chain before [`ListOrdersBuilder::send`].
    pub fn list(
        &self,
        from_entered_time: DateTime<Utc>,
        to_entered_time: DateTime<Utc>,
    ) -> ListOrdersBuilder<'a, 'b> {
        ListOrdersBuilder {
            client: self.client,
            account_hash: self.account_hash,
            from_entered_time,
            to_entered_time,
            max_results: None,
            status: None,
        }
    }
}

/// Accessor for `/orders` (across every linked account). Construct via
/// [`SchwabClient::orders_all`].
pub struct AllOrders<'a> {
    client: &'a SchwabClient,
}

impl<'a> AllOrders<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Begin a `GET /orders` request.
    ///
    /// `from_entered_time` and `to_entered_time` bound the result window.
    /// Schwab caps the window at 60 days for the cross-account endpoint;
    /// this builder does not enforce that.
    pub fn list(
        &self,
        from_entered_time: DateTime<Utc>,
        to_entered_time: DateTime<Utc>,
    ) -> ListAllOrdersBuilder<'a> {
        ListAllOrdersBuilder {
            client: self.client,
            from_entered_time,
            to_entered_time,
            max_results: None,
            status: None,
        }
    }
}

// --- Builders ---

#[must_use = "call .send() to execute the request"]
pub struct ListOrdersBuilder<'a, 'b> {
    client: &'a SchwabClient,
    account_hash: &'b AccountHash,
    from_entered_time: DateTime<Utc>,
    to_entered_time: DateTime<Utc>,
    max_results: Option<i64>,
    status: Option<ApiOrderStatus>,
}

impl<'a, 'b> ListOrdersBuilder<'a, 'b> {
    /// Cap the response size. Schwab's default is 3000.
    pub fn max_results(mut self, n: i64) -> Self {
        self.max_results = Some(n);
        self
    }

    /// Restrict the response to orders in a specific status.
    pub fn status(mut self, status: ApiOrderStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub async fn send(self) -> Result<Vec<Order>> {
        let hash = self.account_hash.expose_secret();
        let from = self
            .from_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let to = self
            .to_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let mut request = self
            .client
            .get(&format!("/accounts/{hash}/orders"))
            .query(&[
                ("fromEnteredTime", from.as_str()),
                ("toEnteredTime", to.as_str()),
            ]);
        if let Some(n) = self.max_results {
            let n_str = n.to_string();
            request = request.query(&[("maxResults", n_str.as_str())]);
        }
        if let Some(s) = self.status {
            let s_str = s.to_string();
            request = request.query(&[("status", s_str.as_str())]);
        }
        self.client.execute_json(request).await
    }
}

#[must_use = "call .send() to execute the request"]
pub struct ListAllOrdersBuilder<'a> {
    client: &'a SchwabClient,
    from_entered_time: DateTime<Utc>,
    to_entered_time: DateTime<Utc>,
    max_results: Option<i64>,
    status: Option<ApiOrderStatus>,
}

impl<'a> ListAllOrdersBuilder<'a> {
    pub fn max_results(mut self, n: i64) -> Self {
        self.max_results = Some(n);
        self
    }

    pub fn status(mut self, status: ApiOrderStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub async fn send(self) -> Result<Vec<Order>> {
        let from = self
            .from_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let to = self
            .to_entered_time
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let mut request = self.client.get("/orders").query(&[
            ("fromEnteredTime", from.as_str()),
            ("toEnteredTime", to.as_str()),
        ]);
        if let Some(n) = self.max_results {
            let n_str = n.to_string();
            request = request.query(&[("maxResults", n_str.as_str())]);
        }
        if let Some(s) = self.status {
            let s_str = s.to_string();
            request = request.query(&[("status", s_str.as_str())]);
        }
        self.client.execute_json(request).await
    }
}

// --- Response shape ---

/// One order, as returned by the read endpoints. Schwab marks almost no
/// field as required, so everything outside the discriminator-bearing
/// enums is `Option`.
///
/// The OpenAPI spec types `accountNumber` and `orderId` as plain `int64`
/// here (in contrast to the string-typed account number on
/// `securitiesAccount`). The fields are kept as numeric here to match.
#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    #[serde(default)]
    pub session: Option<Session>,
    #[serde(default)]
    pub duration: Option<Duration>,
    #[serde(default, rename = "orderType")]
    pub order_type: Option<OrderType>,
    #[serde(default, rename = "cancelTime")]
    pub cancel_time: Option<DateTime<Utc>>,
    #[serde(default, rename = "complexOrderStrategyType")]
    pub complex_order_strategy_type: Option<ComplexOrderStrategyType>,
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "filledQuantity")]
    pub filled_quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "remainingQuantity")]
    pub remaining_quantity: Option<Decimal>,
    /// Response-only: the venue Schwab routed the order to.
    #[serde(default, rename = "requestedDestination")]
    pub requested_destination: Option<RequestedDestination>,
    #[serde(default, rename = "destinationLinkName")]
    pub destination_link_name: Option<String>,
    #[serde(default, rename = "releaseTime")]
    pub release_time: Option<DateTime<Utc>>,
    #[serde(default, with = "decimal_opt", rename = "stopPrice")]
    pub stop_price: Option<Decimal>,
    #[serde(default, rename = "stopPriceLinkBasis")]
    pub stop_price_link_basis: Option<StopPriceLinkBasis>,
    #[serde(default, rename = "stopPriceLinkType")]
    pub stop_price_link_type: Option<StopPriceLinkType>,
    #[serde(default, with = "decimal_opt", rename = "stopPriceOffset")]
    pub stop_price_offset: Option<Decimal>,
    #[serde(default, rename = "stopType")]
    pub stop_type: Option<StopType>,
    #[serde(default, rename = "priceLinkBasis")]
    pub price_link_basis: Option<PriceLinkBasis>,
    #[serde(default, rename = "priceLinkType")]
    pub price_link_type: Option<PriceLinkType>,
    #[serde(default, with = "decimal_opt")]
    pub price: Option<Decimal>,
    #[serde(default, rename = "taxLotMethod")]
    pub tax_lot_method: Option<TaxLotMethod>,
    #[serde(default, rename = "orderLegCollection")]
    pub order_leg_collection: Vec<OrderLegCollection>,
    #[serde(default, with = "decimal_opt", rename = "activationPrice")]
    pub activation_price: Option<Decimal>,
    #[serde(default, rename = "specialInstruction")]
    pub special_instruction: Option<SpecialInstruction>,
    #[serde(default, rename = "orderStrategyType")]
    pub order_strategy_type: Option<OrderStrategyType>,
    #[serde(default, rename = "orderId")]
    pub order_id: Option<i64>,
    #[serde(default)]
    pub cancelable: Option<bool>,
    #[serde(default)]
    pub editable: Option<bool>,
    #[serde(default)]
    pub status: Option<ApiOrderStatus>,
    #[serde(default, rename = "enteredTime")]
    pub entered_time: Option<DateTime<Utc>>,
    #[serde(default, rename = "closeTime")]
    pub close_time: Option<DateTime<Utc>>,
    /// Response-only: Schwab-assigned classification of the order's origin.
    /// Not settable on the request; consumers cannot use this for
    /// client-side correlation.
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default, rename = "accountNumber")]
    pub account_number: Option<i64>,
    #[serde(default, rename = "orderActivityCollection")]
    pub order_activity_collection: Vec<OrderActivity>,
    #[serde(default, rename = "replacingOrderCollection")]
    pub replacing_order_collection: Vec<Order>,
    #[serde(default, rename = "childOrderStrategies")]
    pub child_order_strategies: Vec<Order>,
    #[serde(default, rename = "statusDescription")]
    pub status_description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrderLegCollection {
    #[serde(default, rename = "orderLegType")]
    pub order_leg_type: Option<OrderLegType>,
    #[serde(default, rename = "legId")]
    pub leg_id: Option<i64>,
    #[serde(default)]
    pub instrument: Option<AccountsInstrument>,
    #[serde(default)]
    pub instruction: Option<Instruction>,
    #[serde(default, rename = "positionEffect")]
    pub position_effect: Option<PositionEffect>,
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(default, rename = "quantityType")]
    pub quantity_type: Option<QuantityType>,
    #[serde(default, rename = "divCapGains")]
    pub div_cap_gains: Option<DivCapGains>,
    #[serde(default, rename = "toSymbol")]
    pub to_symbol: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrderActivity {
    #[serde(default, rename = "activityType")]
    pub activity_type: Option<OrderActivityType>,
    #[serde(default, rename = "executionType")]
    pub execution_type: Option<ExecutionType>,
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "orderRemainingQuantity")]
    pub order_remaining_quantity: Option<Decimal>,
    #[serde(default, rename = "executionLegs")]
    pub execution_legs: Vec<ExecutionLeg>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecutionLeg {
    #[serde(default, rename = "legId")]
    pub leg_id: Option<i64>,
    #[serde(default, with = "decimal_opt")]
    pub price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "mismarkedQuantity")]
    pub mismarked_quantity: Option<Decimal>,
    #[serde(default, rename = "instrumentId")]
    pub instrument_id: Option<i64>,
    #[serde(default)]
    pub time: Option<DateTime<Utc>>,
}

// --- Enums (forward-compat via strum default) ---

/// Macro: declare a string-enum with a `Unknown(String)` catch-all so wire
/// values added after this crate was published deserialize cleanly.
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident = $wire:literal ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash,
            strum::Display, strum::EnumString,
            Serialize, Deserialize,
        )]
        #[serde(into = "String", from = "String")]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                #[strum(serialize = $wire)]
                $variant,
            )*
            #[strum(default)]
            Unknown(String),
        }

        impl From<$name> for String {
            fn from(v: $name) -> Self { v.to_string() }
        }

        impl From<String> for $name {
            fn from(v: String) -> Self {
                v.parse().expect(concat!(stringify!($name), " FromStr is infallible (strum default)"))
            }
        }
    };
}

string_enum! {
    Session {
        Normal = "NORMAL",
        Am = "AM",
        Pm = "PM",
        Seamless = "SEAMLESS",
    }
}

string_enum! {
    Duration {
        Day = "DAY",
        GoodTillCancel = "GOOD_TILL_CANCEL",
        FillOrKill = "FILL_OR_KILL",
        ImmediateOrCancel = "IMMEDIATE_OR_CANCEL",
        EndOfWeek = "END_OF_WEEK",
        EndOfMonth = "END_OF_MONTH",
        NextEndOfMonth = "NEXT_END_OF_MONTH",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    OrderType {
        Market = "MARKET",
        Limit = "LIMIT",
        Stop = "STOP",
        StopLimit = "STOP_LIMIT",
        TrailingStop = "TRAILING_STOP",
        Cabinet = "CABINET",
        NonMarketable = "NON_MARKETABLE",
        MarketOnClose = "MARKET_ON_CLOSE",
        Exercise = "EXERCISE",
        TrailingStopLimit = "TRAILING_STOP_LIMIT",
        NetDebit = "NET_DEBIT",
        NetCredit = "NET_CREDIT",
        NetZero = "NET_ZERO",
        LimitOnClose = "LIMIT_ON_CLOSE",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    OrderStrategyType {
        Single = "SINGLE",
        Cancel = "CANCEL",
        Recall = "RECALL",
        Pair = "PAIR",
        Flatten = "FLATTEN",
        TwoDaySwap = "TWO_DAY_SWAP",
        BlastAll = "BLAST_ALL",
        Oco = "OCO",
        Trigger = "TRIGGER",
    }
}

string_enum! {
    ComplexOrderStrategyType {
        None = "NONE",
        Covered = "COVERED",
        Vertical = "VERTICAL",
        BackRatio = "BACK_RATIO",
        Calendar = "CALENDAR",
        Diagonal = "DIAGONAL",
        Straddle = "STRADDLE",
        Strangle = "STRANGLE",
        CollarSynthetic = "COLLAR_SYNTHETIC",
        Butterfly = "BUTTERFLY",
        Condor = "CONDOR",
        IronCondor = "IRON_CONDOR",
        VerticalRoll = "VERTICAL_ROLL",
        CollarWithStock = "COLLAR_WITH_STOCK",
        DoubleDiagonal = "DOUBLE_DIAGONAL",
        UnbalancedButterfly = "UNBALANCED_BUTTERFLY",
        UnbalancedCondor = "UNBALANCED_CONDOR",
        UnbalancedIronCondor = "UNBALANCED_IRON_CONDOR",
        UnbalancedVerticalRoll = "UNBALANCED_VERTICAL_ROLL",
        MutualFundSwap = "MUTUAL_FUND_SWAP",
        Custom = "CUSTOM",
    }
}

string_enum! {
    Instruction {
        Buy = "BUY",
        Sell = "SELL",
        BuyToCover = "BUY_TO_COVER",
        SellShort = "SELL_SHORT",
        BuyToOpen = "BUY_TO_OPEN",
        BuyToClose = "BUY_TO_CLOSE",
        SellToOpen = "SELL_TO_OPEN",
        SellToClose = "SELL_TO_CLOSE",
        Exchange = "EXCHANGE",
        SellShortExempt = "SELL_SHORT_EXEMPT",
    }
}

string_enum! {
    ApiOrderStatus {
        AwaitingParentOrder = "AWAITING_PARENT_ORDER",
        AwaitingCondition = "AWAITING_CONDITION",
        AwaitingStopCondition = "AWAITING_STOP_CONDITION",
        AwaitingManualReview = "AWAITING_MANUAL_REVIEW",
        Accepted = "ACCEPTED",
        AwaitingUrOut = "AWAITING_UR_OUT",
        PendingActivation = "PENDING_ACTIVATION",
        Queued = "QUEUED",
        Working = "WORKING",
        Rejected = "REJECTED",
        PendingCancel = "PENDING_CANCEL",
        Canceled = "CANCELED",
        PendingReplace = "PENDING_REPLACE",
        Replaced = "REPLACED",
        Filled = "FILLED",
        Expired = "EXPIRED",
        New = "NEW",
        AwaitingReleaseTime = "AWAITING_RELEASE_TIME",
        PendingAcknowledgement = "PENDING_ACKNOWLEDGEMENT",
        PendingRecall = "PENDING_RECALL",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    StopType {
        Standard = "STANDARD",
        Bid = "BID",
        Ask = "ASK",
        Last = "LAST",
        Mark = "MARK",
    }
}

string_enum! {
    StopPriceLinkBasis {
        Manual = "MANUAL",
        Base = "BASE",
        Trigger = "TRIGGER",
        Last = "LAST",
        Bid = "BID",
        Ask = "ASK",
        AskBid = "ASK_BID",
        Mark = "MARK",
        Average = "AVERAGE",
    }
}

string_enum! {
    StopPriceLinkType {
        Value = "VALUE",
        Percent = "PERCENT",
        Tick = "TICK",
    }
}

string_enum! {
    PriceLinkBasis {
        Manual = "MANUAL",
        Base = "BASE",
        Trigger = "TRIGGER",
        Last = "LAST",
        Bid = "BID",
        Ask = "ASK",
        AskBid = "ASK_BID",
        Mark = "MARK",
        Average = "AVERAGE",
    }
}

string_enum! {
    PriceLinkType {
        Value = "VALUE",
        Percent = "PERCENT",
        Tick = "TICK",
    }
}

string_enum! {
    TaxLotMethod {
        Fifo = "FIFO",
        Lifo = "LIFO",
        HighCost = "HIGH_COST",
        LowCost = "LOW_COST",
        AverageCost = "AVERAGE_COST",
        SpecificLot = "SPECIFIC_LOT",
        LossHarvester = "LOSS_HARVESTER",
    }
}

string_enum! {
    SpecialInstruction {
        AllOrNone = "ALL_OR_NONE",
        DoNotReduce = "DO_NOT_REDUCE",
        AllOrNoneDoNotReduce = "ALL_OR_NONE_DO_NOT_REDUCE",
    }
}

string_enum! {
    RequestedDestination {
        Inet = "INET",
        EcnArca = "ECN_ARCA",
        Cboe = "CBOE",
        Amex = "AMEX",
        Phlx = "PHLX",
        Ise = "ISE",
        Box_ = "BOX",
        Nyse = "NYSE",
        Nasdaq = "NASDAQ",
        Bats = "BATS",
        C2 = "C2",
        Auto = "AUTO",
    }
}

string_enum! {
    OrderLegType {
        Equity = "EQUITY",
        Option = "OPTION",
        Index = "INDEX",
        MutualFund = "MUTUAL_FUND",
        CashEquivalent = "CASH_EQUIVALENT",
        FixedIncome = "FIXED_INCOME",
        Currency = "CURRENCY",
        CollectiveInvestment = "COLLECTIVE_INVESTMENT",
    }
}

string_enum! {
    PositionEffect {
        Opening = "OPENING",
        Closing = "CLOSING",
        Automatic = "AUTOMATIC",
    }
}

string_enum! {
    QuantityType {
        AllShares = "ALL_SHARES",
        Dollars = "DOLLARS",
        Shares = "SHARES",
    }
}

string_enum! {
    DivCapGains {
        Reinvest = "REINVEST",
        Payout = "PAYOUT",
    }
}

string_enum! {
    OrderActivityType {
        Execution = "EXECUTION",
        OrderAction = "ORDER_ACTION",
    }
}

string_enum! {
    ExecutionType {
        Fill = "FILL",
    }
}

// --- Request shape ---

/// Body of `POST /accounts/{accountNumber}/orders` (place) and
/// `PUT /accounts/{accountNumber}/orders/{orderId}` (replace, in a later
/// slice). Construct via [`OrderRequest::single`] (typestate builder) or
/// by populating fields directly.
///
/// Response-only fields (`status`, `filledQuantity`, `enteredTime`,
/// `tag`, `requestedDestination`, etc.) are not present here; they live
/// on [`Order`] instead.
#[derive(Debug, Clone, Default, Serialize)]
pub struct OrderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<Session>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<Duration>,
    #[serde(rename = "orderType", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<OrderType>,
    #[serde(
        rename = "complexOrderStrategyType",
        skip_serializing_if = "Option::is_none"
    )]
    pub complex_order_strategy_type: Option<ComplexOrderStrategyType>,
    #[serde(skip_serializing_if = "Option::is_none", with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(
        rename = "destinationLinkName",
        skip_serializing_if = "Option::is_none"
    )]
    pub destination_link_name: Option<String>,
    #[serde(
        rename = "stopPrice",
        skip_serializing_if = "Option::is_none",
        with = "decimal_opt"
    )]
    pub stop_price: Option<Decimal>,
    #[serde(rename = "stopPriceLinkBasis", skip_serializing_if = "Option::is_none")]
    pub stop_price_link_basis: Option<StopPriceLinkBasis>,
    #[serde(rename = "stopPriceLinkType", skip_serializing_if = "Option::is_none")]
    pub stop_price_link_type: Option<StopPriceLinkType>,
    #[serde(
        rename = "stopPriceOffset",
        skip_serializing_if = "Option::is_none",
        with = "decimal_opt"
    )]
    pub stop_price_offset: Option<Decimal>,
    #[serde(rename = "stopType", skip_serializing_if = "Option::is_none")]
    pub stop_type: Option<StopType>,
    #[serde(rename = "priceLinkBasis", skip_serializing_if = "Option::is_none")]
    pub price_link_basis: Option<PriceLinkBasis>,
    #[serde(rename = "priceLinkType", skip_serializing_if = "Option::is_none")]
    pub price_link_type: Option<PriceLinkType>,
    #[serde(skip_serializing_if = "Option::is_none", with = "decimal_opt")]
    pub price: Option<Decimal>,
    #[serde(rename = "taxLotMethod", skip_serializing_if = "Option::is_none")]
    pub tax_lot_method: Option<TaxLotMethod>,
    #[serde(rename = "orderLegCollection", skip_serializing_if = "Vec::is_empty")]
    pub order_leg_collection: Vec<OrderLegRequest>,
    #[serde(
        rename = "activationPrice",
        skip_serializing_if = "Option::is_none",
        with = "decimal_opt"
    )]
    pub activation_price: Option<Decimal>,
    #[serde(rename = "specialInstruction", skip_serializing_if = "Option::is_none")]
    pub special_instruction: Option<SpecialInstruction>,
    #[serde(rename = "orderStrategyType", skip_serializing_if = "Option::is_none")]
    pub order_strategy_type: Option<OrderStrategyType>,
    #[serde(rename = "childOrderStrategies", skip_serializing_if = "Vec::is_empty")]
    pub child_order_strategies: Vec<OrderRequest>,
}

/// One leg of an [`OrderRequest`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct OrderLegRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<Instruction>,
    #[serde(skip_serializing_if = "Option::is_none", with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<OrderInstrumentRequest>,
    #[serde(rename = "positionEffect", skip_serializing_if = "Option::is_none")]
    pub position_effect: Option<PositionEffect>,
    #[serde(rename = "quantityType", skip_serializing_if = "Option::is_none")]
    pub quantity_type: Option<QuantityType>,
}

/// Minimal request-side instrument: only `symbol` and `assetType` are
/// settable. Use the typed [`AssetType`] from [`crate::api::accounts`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct OrderInstrumentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(rename = "assetType", skip_serializing_if = "Option::is_none")]
    pub asset_type: Option<AssetType>,
}

// --- Typestate builder for SINGLE-strategy orders ---

/// Builder state: order type (market / limit / etc.) has not been set yet.
pub struct NeedsType;
/// Builder state: order type is set; at least one leg must still be added.
pub struct NeedsLeg;
/// Builder state: at least one leg has been added. Optional fields may be
/// set, additional legs may be appended (for multi-leg single orders such
/// as vertical spreads), and `.build()` is callable.
pub struct Ready;

/// Trait used to lift leg-adding methods across the two states that
/// accept legs (`NeedsLeg` → `Ready`, and `Ready` → `Ready` for
/// multi-leg orders). Not part of the public API surface; the associated
/// type lets one set of method definitions serve both transitions.
pub trait AcceptsLeg: sealed::Sealed {
    /// State to transition into after a leg is added.
    type AfterLeg;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::NeedsLeg {}
    impl Sealed for super::Ready {}
}

impl AcceptsLeg for NeedsLeg {
    type AfterLeg = Ready;
}

impl AcceptsLeg for Ready {
    type AfterLeg = Ready;
}

/// Typestate builder for a `SINGLE` strategy order. Construct via
/// [`OrderRequest::single`].
#[must_use = "call .build() to finalize the OrderRequest"]
pub struct SingleOrderBuilder<State> {
    inner: OrderRequest,
    _state: PhantomData<State>,
}

impl OrderRequest {
    /// Begin building a `SINGLE` strategy order. Defaults `session=NORMAL`
    /// and `duration=DAY`; override with [`SingleOrderBuilder::session`]
    /// and [`SingleOrderBuilder::duration`] on the [`Ready`] state.
    pub fn single() -> SingleOrderBuilder<NeedsType> {
        let inner = OrderRequest {
            session: Some(Session::Normal),
            duration: Some(Duration::Day),
            order_strategy_type: Some(OrderStrategyType::Single),
            ..Default::default()
        };
        SingleOrderBuilder {
            inner,
            _state: PhantomData,
        }
    }
}

impl SingleOrderBuilder<NeedsType> {
    /// Market order.
    pub fn market(mut self) -> SingleOrderBuilder<NeedsLeg> {
        self.inner.order_type = Some(OrderType::Market);
        self.transition()
    }

    /// Limit order at `price`.
    pub fn limit(mut self, price: Decimal) -> SingleOrderBuilder<NeedsLeg> {
        self.inner.order_type = Some(OrderType::Limit);
        self.inner.price = Some(price);
        self.transition()
    }

    /// Stop (stop-market) order at `stop_price`.
    pub fn stop(mut self, stop_price: Decimal) -> SingleOrderBuilder<NeedsLeg> {
        self.inner.order_type = Some(OrderType::Stop);
        self.inner.stop_price = Some(stop_price);
        self.transition()
    }

    /// Stop-limit order: triggered when the market crosses `stop_price`,
    /// then becomes a limit order at `limit_price`.
    pub fn stop_limit(
        mut self,
        stop_price: Decimal,
        limit_price: Decimal,
    ) -> SingleOrderBuilder<NeedsLeg> {
        self.inner.order_type = Some(OrderType::StopLimit);
        self.inner.stop_price = Some(stop_price);
        self.inner.price = Some(limit_price);
        self.transition()
    }

    /// Net-debit order (multi-leg options, debit spread). The `price` is
    /// the net premium paid.
    pub fn net_debit(mut self, price: Decimal) -> SingleOrderBuilder<NeedsLeg> {
        self.inner.order_type = Some(OrderType::NetDebit);
        self.inner.price = Some(price);
        self.transition()
    }

    /// Net-credit order (multi-leg options, credit spread). The `price`
    /// is the net premium received.
    pub fn net_credit(mut self, price: Decimal) -> SingleOrderBuilder<NeedsLeg> {
        self.inner.order_type = Some(OrderType::NetCredit);
        self.inner.price = Some(price);
        self.transition()
    }

    fn transition(self) -> SingleOrderBuilder<NeedsLeg> {
        SingleOrderBuilder {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl<S: AcceptsLeg> SingleOrderBuilder<S> {
    fn push_leg(mut self, leg: OrderLegRequest) -> SingleOrderBuilder<S::AfterLeg> {
        self.inner.order_leg_collection.push(leg);
        SingleOrderBuilder {
            inner: self.inner,
            _state: PhantomData,
        }
    }

    /// Buy `qty` shares of `symbol` (equity).
    pub fn equity_buy(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(equity_leg(Instruction::Buy, symbol, qty))
    }

    /// Sell `qty` shares of `symbol` (equity, long sale).
    pub fn equity_sell(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(equity_leg(Instruction::Sell, symbol, qty))
    }

    /// Short-sell `qty` shares of `symbol` (equity).
    pub fn equity_sell_short(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(equity_leg(Instruction::SellShort, symbol, qty))
    }

    /// Buy to cover (close a short) `qty` shares of `symbol`.
    pub fn equity_buy_to_cover(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(equity_leg(Instruction::BuyToCover, symbol, qty))
    }

    /// Buy to open `qty` contracts of `symbol` (option).
    pub fn option_buy_to_open(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(option_leg(Instruction::BuyToOpen, symbol, qty))
    }

    /// Sell to open `qty` contracts of `symbol` (option).
    pub fn option_sell_to_open(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(option_leg(Instruction::SellToOpen, symbol, qty))
    }

    /// Buy to close `qty` contracts of `symbol` (option).
    pub fn option_buy_to_close(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(option_leg(Instruction::BuyToClose, symbol, qty))
    }

    /// Sell to close `qty` contracts of `symbol` (option).
    pub fn option_sell_to_close(
        self,
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<S::AfterLeg> {
        self.push_leg(option_leg(Instruction::SellToClose, symbol, qty))
    }
}

impl SingleOrderBuilder<Ready> {
    /// Override the default `DAY` duration.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.inner.duration = Some(duration);
        self
    }

    /// Override the default `NORMAL` session.
    pub fn session(mut self, session: Session) -> Self {
        self.inner.session = Some(session);
        self
    }

    /// Attach a special instruction (e.g. `ALL_OR_NONE`).
    pub fn special_instruction(mut self, instr: SpecialInstruction) -> Self {
        self.inner.special_instruction = Some(instr);
        self
    }

    /// Set the complex-order-strategy type (defaults to absent, which
    /// Schwab interprets as `NONE`). Useful for option spreads.
    pub fn complex_order_strategy_type(mut self, t: ComplexOrderStrategyType) -> Self {
        self.inner.complex_order_strategy_type = Some(t);
        self
    }

    pub fn build(self) -> OrderRequest {
        self.inner
    }
}

fn equity_leg(
    instruction: Instruction,
    symbol: impl Into<String>,
    qty: Decimal,
) -> OrderLegRequest {
    OrderLegRequest {
        instruction: Some(instruction),
        quantity: Some(qty),
        instrument: Some(OrderInstrumentRequest {
            symbol: Some(symbol.into()),
            asset_type: Some(AssetType::Equity),
        }),
        ..Default::default()
    }
}

fn option_leg(
    instruction: Instruction,
    symbol: impl Into<String>,
    qty: Decimal,
) -> OrderLegRequest {
    OrderLegRequest {
        instruction: Some(instruction),
        quantity: Some(qty),
        instrument: Some(OrderInstrumentRequest {
            symbol: Some(symbol.into()),
            asset_type: Some(AssetType::Option),
        }),
        ..Default::default()
    }
}

// --- Location header parsing ---

/// Parse Schwab's `Location` header after a successful place/replace and
/// extract the trailing `{orderId}` segment. Accepts both absolute URLs
/// (`https://api.schwabapi.com/.../orders/123`) and bare paths.
fn parse_order_id_from_location(response: &reqwest::Response) -> Result<i64> {
    let value = response
        .headers()
        .get(reqwest::header::LOCATION)
        .ok_or(Error::MissingLocationHeader)?
        .to_str()
        .map_err(|e| Error::InvalidLocationHeader(format!("not ASCII: {e}")))?;
    parse_order_id_from_location_str(value)
}

fn parse_order_id_from_location_str(location: &str) -> Result<i64> {
    // Strip any trailing slash, then take the last path segment.
    let trimmed = location.trim_end_matches('/');
    let id_segment = trimmed
        .rsplit('/')
        .next()
        .ok_or_else(|| Error::InvalidLocationHeader(location.to_string()))?;
    // Strip a possible query string.
    let id_segment = id_segment.split(['?', '#']).next().unwrap_or(id_segment);
    id_segment
        .parse::<i64>()
        .map_err(|_| Error::InvalidLocationHeader(location.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn filled_equity_order_parses_with_execution() {
        let json = r#"{
            "orderId": 100000001,
            "accountNumber": 12345678,
            "status": "FILLED",
            "orderType": "LIMIT",
            "session": "NORMAL",
            "duration": "DAY",
            "orderStrategyType": "SINGLE",
            "complexOrderStrategyType": "NONE",
            "quantity": 10.0,
            "filledQuantity": 10.0,
            "remainingQuantity": 0.0,
            "price": 145.32,
            "enteredTime": "2024-03-15T15:30:00.000Z",
            "closeTime": "2024-03-15T15:30:02.500Z",
            "cancelable": false,
            "editable": false,
            "orderLegCollection": [{
                "orderLegType": "EQUITY",
                "legId": 1,
                "instruction": "BUY",
                "positionEffect": "OPENING",
                "quantity": 10.0,
                "quantityType": "SHARES",
                "instrument": {
                    "assetType": "EQUITY",
                    "symbol": "AAPL",
                    "cusip": "037833100",
                    "instrumentId": 12345
                }
            }],
            "orderActivityCollection": [{
                "activityType": "EXECUTION",
                "executionType": "FILL",
                "quantity": 10.0,
                "orderRemainingQuantity": 0.0,
                "executionLegs": [{
                    "legId": 1,
                    "price": 145.32,
                    "quantity": 10.0,
                    "mismarkedQuantity": 0.0,
                    "instrumentId": 12345,
                    "time": "2024-03-15T15:30:02.500Z"
                }]
            }]
        }"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert_eq!(order.order_id, Some(100000001));
        assert_eq!(order.status, Some(ApiOrderStatus::Filled));
        assert_eq!(order.order_type, Some(OrderType::Limit));
        assert_eq!(order.order_strategy_type, Some(OrderStrategyType::Single));
        assert_eq!(order.quantity, Some(dec!(10.0)));
        assert_eq!(order.filled_quantity, Some(dec!(10.0)));
        assert_eq!(order.price, Some(dec!(145.32)));
        assert_eq!(order.cancelable, Some(false));

        assert_eq!(order.order_leg_collection.len(), 1);
        let leg = &order.order_leg_collection[0];
        assert_eq!(leg.instruction, Some(Instruction::Buy));
        assert_eq!(leg.position_effect, Some(PositionEffect::Opening));
        assert_eq!(leg.quantity, Some(dec!(10.0)));
        assert_eq!(leg.quantity_type, Some(QuantityType::Shares));

        assert_eq!(order.order_activity_collection.len(), 1);
        let activity = &order.order_activity_collection[0];
        assert_eq!(activity.activity_type, Some(OrderActivityType::Execution));
        assert_eq!(activity.execution_type, Some(ExecutionType::Fill));
        assert_eq!(activity.execution_legs.len(), 1);
        let exec = &activity.execution_legs[0];
        assert_eq!(exec.price, Some(dec!(145.32)));
        assert_eq!(exec.quantity, Some(dec!(10.0)));
    }

    #[test]
    fn working_order_with_no_fills_parses() {
        let json = r#"{
            "orderId": 100000002,
            "status": "WORKING",
            "orderType": "LIMIT",
            "orderStrategyType": "SINGLE",
            "quantity": 5.0,
            "filledQuantity": 0.0,
            "remainingQuantity": 5.0,
            "price": 140.00,
            "cancelable": true,
            "editable": true,
            "orderLegCollection": [{
                "orderLegType": "EQUITY",
                "instruction": "BUY",
                "quantity": 5.0,
                "instrument": {
                    "assetType": "EQUITY",
                    "symbol": "AAPL"
                }
            }]
        }"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert_eq!(order.status, Some(ApiOrderStatus::Working));
        assert_eq!(order.filled_quantity, Some(dec!(0.0)));
        assert_eq!(order.remaining_quantity, Some(dec!(5.0)));
        assert!(order.order_activity_collection.is_empty());
        assert_eq!(order.cancelable, Some(true));
    }

    #[test]
    fn trigger_strategy_parses_with_child_orders() {
        let json = r#"{
            "orderId": 100000003,
            "orderStrategyType": "TRIGGER",
            "orderType": "LIMIT",
            "price": 34.97,
            "quantity": 10.0,
            "orderLegCollection": [{
                "instruction": "BUY",
                "quantity": 10.0,
                "instrument": { "assetType": "EQUITY", "symbol": "XYZ" }
            }],
            "childOrderStrategies": [{
                "orderId": 100000004,
                "orderStrategyType": "SINGLE",
                "orderType": "LIMIT",
                "price": 42.03,
                "quantity": 10.0,
                "orderLegCollection": [{
                    "instruction": "SELL",
                    "quantity": 10.0,
                    "instrument": { "assetType": "EQUITY", "symbol": "XYZ" }
                }]
            }]
        }"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert_eq!(order.order_strategy_type, Some(OrderStrategyType::Trigger));
        assert_eq!(order.child_order_strategies.len(), 1);
        let child = &order.child_order_strategies[0];
        assert_eq!(child.order_id, Some(100000004));
        assert_eq!(child.order_strategy_type, Some(OrderStrategyType::Single));
        assert_eq!(child.price, Some(dec!(42.03)));
    }

    #[test]
    fn unknown_status_preserves_raw_string() {
        let parsed: ApiOrderStatus = serde_json::from_str(r#""SOME_NEW_STATE""#).unwrap();
        assert!(matches!(parsed, ApiOrderStatus::Unknown(ref s) if s == "SOME_NEW_STATE"));
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            r#""SOME_NEW_STATE""#
        );
    }

    #[test]
    fn unknown_order_type_preserves_raw_string() {
        let parsed: OrderType = serde_json::from_str(r#""NEW_TYPE""#).unwrap();
        assert!(matches!(parsed, OrderType::Unknown(ref s) if s == "NEW_TYPE"));
    }

    #[test]
    fn order_status_round_trips_each_known_variant() {
        for raw in [
            "AWAITING_PARENT_ORDER",
            "ACCEPTED",
            "WORKING",
            "FILLED",
            "REJECTED",
            "CANCELED",
            "EXPIRED",
            "PENDING_CANCEL",
            "PENDING_REPLACE",
            "REPLACED",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: ApiOrderStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn empty_collections_default_to_empty_vecs() {
        let json = r#"{"orderId": 1}"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert!(order.order_leg_collection.is_empty());
        assert!(order.order_activity_collection.is_empty());
        assert!(order.child_order_strategies.is_empty());
        assert!(order.replacing_order_collection.is_empty());
    }

    // --- Builder & request serialization ---

    fn pretty(value: &serde_json::Value) -> String {
        serde_json::to_string_pretty(value).unwrap()
    }

    #[test]
    fn builder_buy_market_equity_matches_schwab_example() {
        // Schwab documented example: "Buy 15 shares of XYZ at the Market
        // good for the Day."
        let req = OrderRequest::single()
            .market()
            .equity_buy("XYZ", dec!(15))
            .build();
        let actual: serde_json::Value = serde_json::to_value(&req).unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "session": "NORMAL",
                "duration": "DAY",
                "orderType": "MARKET",
                "orderStrategyType": "SINGLE",
                "orderLegCollection": [{
                    "instruction": "BUY",
                    "quantity": 15,
                    "instrument": {
                        "symbol": "XYZ",
                        "assetType": "EQUITY"
                    }
                }]
            }"#,
        )
        .unwrap();
        assert_eq!(actual, expected, "got: {}", pretty(&actual));
    }

    #[test]
    fn builder_buy_limit_option_matches_schwab_example() {
        // "Buy to open 10 contracts of the XYZ March 15, 2024 $50 CALL at
        // a Limit of $6.45 good for the Day."
        let req = OrderRequest::single()
            .limit(dec!(6.45))
            .option_buy_to_open("XYZ   240315C00500000", dec!(10))
            .complex_order_strategy_type(ComplexOrderStrategyType::None)
            .build();
        let actual: serde_json::Value = serde_json::to_value(&req).unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "complexOrderStrategyType": "NONE",
                "orderType": "LIMIT",
                "session": "NORMAL",
                "price": 6.45,
                "duration": "DAY",
                "orderStrategyType": "SINGLE",
                "orderLegCollection": [{
                    "instruction": "BUY_TO_OPEN",
                    "quantity": 10,
                    "instrument": {
                        "symbol": "XYZ   240315C00500000",
                        "assetType": "OPTION"
                    }
                }]
            }"#,
        )
        .unwrap();
        assert_eq!(actual, expected, "got: {}", pretty(&actual));
    }

    #[test]
    fn builder_vertical_spread_uses_net_debit_with_two_legs() {
        // "Buy to open 2 XYZ Mar 15 2024 $45 Put, Sell to open 2 XYZ Mar
        // 15 2024 $43 Put at LIMIT $0.10 good for the Day."
        let req = OrderRequest::single()
            .net_debit(dec!(0.10))
            .option_buy_to_open("XYZ   240315P00045000", dec!(2))
            .option_sell_to_open("XYZ   240315P00043000", dec!(2))
            .build();
        let actual: serde_json::Value = serde_json::to_value(&req).unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "orderType": "NET_DEBIT",
                "session": "NORMAL",
                "price": 0.10,
                "duration": "DAY",
                "orderStrategyType": "SINGLE",
                "orderLegCollection": [
                    {
                        "instruction": "BUY_TO_OPEN",
                        "quantity": 2,
                        "instrument": {
                            "symbol": "XYZ   240315P00045000",
                            "assetType": "OPTION"
                        }
                    },
                    {
                        "instruction": "SELL_TO_OPEN",
                        "quantity": 2,
                        "instrument": {
                            "symbol": "XYZ   240315P00043000",
                            "assetType": "OPTION"
                        }
                    }
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(actual, expected, "got: {}", pretty(&actual));
    }

    #[test]
    fn builder_optional_setters_override_defaults() {
        let req = OrderRequest::single()
            .limit(dec!(140.00))
            .equity_buy("AAPL", dec!(5))
            .duration(Duration::GoodTillCancel)
            .session(Session::Seamless)
            .special_instruction(SpecialInstruction::AllOrNone)
            .build();
        assert_eq!(req.duration, Some(Duration::GoodTillCancel));
        assert_eq!(req.session, Some(Session::Seamless));
        assert_eq!(req.special_instruction, Some(SpecialInstruction::AllOrNone));
    }

    #[test]
    fn builder_serialization_omits_response_only_fields() {
        let req = OrderRequest::single()
            .market()
            .equity_buy("AAPL", dec!(1))
            .build();
        let json = serde_json::to_string(&req).unwrap();
        // None of the response-only fields should appear in the request body.
        for forbidden in [
            "status",
            "orderId",
            "accountNumber",
            "tag",
            "requestedDestination",
            "filledQuantity",
            "remainingQuantity",
            "enteredTime",
            "closeTime",
            "cancelable",
            "editable",
            "orderActivityCollection",
        ] {
            assert!(
                !json.contains(forbidden),
                "request body should not contain {forbidden}, got: {json}"
            );
        }
    }

    // --- Location header parsing ---

    #[test]
    fn parse_order_id_from_absolute_url() {
        let id = parse_order_id_from_location_str(
            "https://api.schwabapi.com/trader/v1/accounts/ABCDEF/orders/100000001",
        )
        .unwrap();
        assert_eq!(id, 100000001);
    }

    #[test]
    fn parse_order_id_from_relative_path() {
        let id = parse_order_id_from_location_str("/trader/v1/accounts/ABCDEF/orders/42").unwrap();
        assert_eq!(id, 42);
    }

    #[test]
    fn parse_order_id_strips_trailing_slash() {
        let id = parse_order_id_from_location_str("/accounts/ABCDEF/orders/99/").unwrap();
        assert_eq!(id, 99);
    }

    #[test]
    fn parse_order_id_strips_query_string() {
        let id = parse_order_id_from_location_str("/accounts/ABCDEF/orders/77?v=1").unwrap();
        assert_eq!(id, 77);
    }

    #[test]
    fn parse_order_id_rejects_non_numeric() {
        let err = parse_order_id_from_location_str("/accounts/ABCDEF/orders/oops").unwrap_err();
        match err {
            Error::InvalidLocationHeader(s) => assert!(s.contains("oops")),
            other => panic!("expected InvalidLocationHeader, got {other:?}"),
        }
    }
}
