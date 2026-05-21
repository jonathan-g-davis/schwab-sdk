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
//! 
//! Callers that need retry-safe submission must implement their own deduplication
//! logic (e.g. by listing orders after a transient failure and matching by
//! entered-time window, symbol, side, and quantity).

use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::{Deserialize, Serialize};

use crate::api::accounts::AccountsInstrument;
use crate::error::Result;
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
}
