//! Request shapes and the typestate builder for constructing them.
//!
//! [`OrderRequest`] is the body of `POST /accounts/{n}/orders` (place) and
//! `PUT /accounts/{n}/orders/{id}` (replace). Construct via
//! [`OrderRequest::single`] for the typed-state builder, or via the
//! composite-strategy factories [`OrderRequest::oco`] and
//! [`OrderRequest::trigger`]. The builder is the only path to a valid
//! request body.
//!
//! # Examples
//!
//! The typestate builder enforces construction order at compile time: an
//! order type before a leg, and at least one leg before `.build()`.
//!
//! ```
//! use rust_decimal_macros::dec;
//! use schwab_sdk::orders::{Duration, OrderRequest, Session};
//!
//! // Limit buy, good-till-cancel, extended session.
//! let order = OrderRequest::single()
//!     .limit(dec!(140.00))
//!     .equity_buy("AAPL", dec!(5))
//!     .duration(Duration::GoodTillCancel)
//!     .session(Session::Seamless)
//!     .build();
//! # let _ = order;
//! ```
//!
//! A two-leg vertical spread chains legs on one order with a net-debit price:
//!
//! ```
//! use rust_decimal_macros::dec;
//! use schwab_sdk::orders::OrderRequest;
//!
//! let spread = OrderRequest::single()
//!     .net_debit(dec!(0.10))
//!     .option_buy_to_open("XYZ   240315P00045000", dec!(2))
//!     .option_sell_to_open("XYZ   240315P00043000", dec!(2))
//!     .build();
//! # let _ = spread;
//! ```

use std::marker::PhantomData;

use rust_decimal::Decimal;
use serde::Serialize;

use crate::accounts::AssetType;
use crate::error::Error;
use crate::orders::enums::{
    ComplexOrderStrategyType, Duration, Instruction, OrderStrategyType, OrderType, PositionEffect,
    PriceLinkBasis, PriceLinkType, QuantityType, Session, SpecialInstruction, StopPriceLinkBasis,
    StopPriceLinkType, StopType, TaxLotMethod,
};
use crate::orders::response::{Order, OrderLegCollection};

/// Local serde helper for `Option<Decimal>` on **request bodies** that
/// preserves the textual form of the decimal value. Read-side helpers can
/// keep using the upstream `float_option` because its deserialize path
/// preserves the string representation already.
mod decimal_opt {
    use rust_decimal::Decimal;
    use serde::{Serialize, Serializer};

    pub fn serialize<S: Serializer>(value: &Option<Decimal>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(d) => {
                let n: serde_json::Number =
                    d.to_string().parse().map_err(serde::ser::Error::custom)?;
                n.serialize(s)
            }
            None => s.serialize_none(),
        }
    }
}

/// Body of `POST /accounts/{accountNumber}/orders` (place) and
/// `PUT /accounts/{accountNumber}/orders/{orderId}` (replace). Construct
/// via [`OrderRequest::single`] (typestate builder) or via the
/// composite-strategy factories [`OrderRequest::oco`] and
/// [`OrderRequest::trigger`].
///
/// Response-only fields (`status`, `filledQuantity`, `enteredTime`,
/// `tag`, `requestedDestination`, etc.) are not present here; they live
/// on [`Order`](crate::orders::Order) instead.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OrderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session: Option<Session>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration: Option<Duration>,
    #[serde(rename = "orderType", skip_serializing_if = "Option::is_none")]
    pub(crate) order_type: Option<OrderType>,
    #[serde(
        rename = "complexOrderStrategyType",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) complex_order_strategy_type: Option<ComplexOrderStrategyType>,
    #[serde(skip_serializing_if = "Option::is_none", with = "decimal_opt")]
    pub(crate) quantity: Option<Decimal>,
    #[serde(
        rename = "destinationLinkName",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) destination_link_name: Option<String>,
    #[serde(
        rename = "stopPrice",
        skip_serializing_if = "Option::is_none",
        with = "decimal_opt"
    )]
    pub(crate) stop_price: Option<Decimal>,
    #[serde(rename = "stopPriceLinkBasis", skip_serializing_if = "Option::is_none")]
    pub(crate) stop_price_link_basis: Option<StopPriceLinkBasis>,
    #[serde(rename = "stopPriceLinkType", skip_serializing_if = "Option::is_none")]
    pub(crate) stop_price_link_type: Option<StopPriceLinkType>,
    #[serde(
        rename = "stopPriceOffset",
        skip_serializing_if = "Option::is_none",
        with = "decimal_opt"
    )]
    pub(crate) stop_price_offset: Option<Decimal>,
    #[serde(rename = "stopType", skip_serializing_if = "Option::is_none")]
    pub(crate) stop_type: Option<StopType>,
    #[serde(rename = "priceLinkBasis", skip_serializing_if = "Option::is_none")]
    pub(crate) price_link_basis: Option<PriceLinkBasis>,
    #[serde(rename = "priceLinkType", skip_serializing_if = "Option::is_none")]
    pub(crate) price_link_type: Option<PriceLinkType>,
    #[serde(skip_serializing_if = "Option::is_none", with = "decimal_opt")]
    pub(crate) price: Option<Decimal>,
    #[serde(rename = "taxLotMethod", skip_serializing_if = "Option::is_none")]
    pub(crate) tax_lot_method: Option<TaxLotMethod>,
    #[serde(rename = "orderLegCollection", skip_serializing_if = "Vec::is_empty")]
    pub(crate) order_leg_collection: Vec<OrderLegRequest>,
    #[serde(
        rename = "activationPrice",
        skip_serializing_if = "Option::is_none",
        with = "decimal_opt"
    )]
    pub(crate) activation_price: Option<Decimal>,
    #[serde(rename = "specialInstruction", skip_serializing_if = "Option::is_none")]
    pub(crate) special_instruction: Option<SpecialInstruction>,
    #[serde(rename = "orderStrategyType", skip_serializing_if = "Option::is_none")]
    pub(crate) order_strategy_type: Option<OrderStrategyType>,
    #[serde(rename = "childOrderStrategies", skip_serializing_if = "Vec::is_empty")]
    pub(crate) child_order_strategies: Vec<OrderRequest>,
}

impl OrderRequest {
    pub(crate) fn empty() -> Self {
        Self {
            session: None,
            duration: None,
            order_type: None,
            complex_order_strategy_type: None,
            quantity: None,
            destination_link_name: None,
            stop_price: None,
            stop_price_link_basis: None,
            stop_price_link_type: None,
            stop_price_offset: None,
            stop_type: None,
            price_link_basis: None,
            price_link_type: None,
            price: None,
            tax_lot_method: None,
            order_leg_collection: Vec::new(),
            activation_price: None,
            special_instruction: None,
            order_strategy_type: None,
            child_order_strategies: Vec::new(),
        }
    }
}

/// One leg of an [`OrderRequest`]. Legs are constructed by the builder's
/// `equity_*` / `option_*` methods.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OrderLegRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) instruction: Option<Instruction>,
    #[serde(skip_serializing_if = "Option::is_none", with = "decimal_opt")]
    pub(crate) quantity: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) instrument: Option<OrderInstrumentRequest>,
    #[serde(rename = "positionEffect", skip_serializing_if = "Option::is_none")]
    pub(crate) position_effect: Option<PositionEffect>,
    #[serde(rename = "quantityType", skip_serializing_if = "Option::is_none")]
    pub(crate) quantity_type: Option<QuantityType>,
}

/// Minimal request-side instrument: only `symbol` and `assetType` are
/// settable. Uses the typed [`AssetType`] from [`crate::accounts`]. Instruments
/// are produced by the builder.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OrderInstrumentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) symbol: Option<String>,
    #[serde(rename = "assetType", skip_serializing_if = "Option::is_none")]
    pub(crate) asset_type: Option<AssetType>,
}

// --- Typestate builder for SINGLE-strategy orders ---

/// Builder state: order type (market / limit / etc.) has not been set yet.
#[derive(Debug)]
pub struct NeedsType;
/// Builder state: order type is set; at least one leg must still be added.
#[derive(Debug)]
pub struct NeedsLeg;
/// Builder state: at least one leg has been added. Optional fields may be
/// set, additional legs may be appended (for multi-leg single orders such
/// as vertical spreads), and `.build()` is callable.
#[derive(Debug)]
pub struct Ready;

/// Trait used to lift leg-adding methods across the two states that
/// accept legs (`NeedsLeg` -> `Ready`, and `Ready` -> `Ready` for
/// multi-leg orders). The associated type lets one set of method
/// definitions serve both transitions.
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
#[derive(Debug)]
#[must_use = "call .build() to finalize the OrderRequest"]
pub struct SingleOrderBuilder<State> {
    inner: OrderRequest,
    _state: PhantomData<State>,
}

impl OrderRequest {
    /// Begin building a `SINGLE` strategy order. Defaults `session=NORMAL`
    /// and `duration=DAY`; override with [`SingleOrderBuilder::session`]
    /// and [`SingleOrderBuilder::duration`] on the `Ready` state.
    pub fn single() -> SingleOrderBuilder<NeedsType> {
        let inner = OrderRequest {
            session: Some(Session::Normal),
            duration: Some(Duration::Day),
            order_strategy_type: Some(OrderStrategyType::Single),
            ..OrderRequest::empty()
        };
        SingleOrderBuilder {
            inner,
            _state: PhantomData,
        }
    }

    // --- Convenience shortcuts for common equity SINGLE orders ---
    //
    // These return a [`SingleOrderBuilder`] in the [`Ready`] state, so
    // callers may chain optional setters (`.duration()`, `.session()`,
    // `.special_instruction()`) and finish with `.build()`. For the
    // simplest case the call site reads `.buy_market(sym, qty).build()`.
    //
    // OCO / TRIGGER factories accept either a built `OrderRequest` or a
    // `SingleOrderBuilder<Ready>` via `impl Into<OrderRequest>`, so the
    // builder can be passed straight through without an explicit `.build()`.

    /// Equity buy-at-market, default day order.
    pub fn buy_market(symbol: impl Into<String>, qty: Decimal) -> SingleOrderBuilder<Ready> {
        Self::single().market().equity_buy(symbol, qty)
    }

    /// Equity buy-at-limit, default day order.
    pub fn buy_limit(
        symbol: impl Into<String>,
        qty: Decimal,
        price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().limit(price).equity_buy(symbol, qty)
    }

    /// Equity long-sale at market, default day order.
    pub fn sell_market(symbol: impl Into<String>, qty: Decimal) -> SingleOrderBuilder<Ready> {
        Self::single().market().equity_sell(symbol, qty)
    }

    /// Equity long-sale at limit, default day order.
    pub fn sell_limit(
        symbol: impl Into<String>,
        qty: Decimal,
        price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().limit(price).equity_sell(symbol, qty)
    }

    /// Equity stop-market sell, default day order. Useful for stop-loss
    /// exits.
    pub fn sell_stop(
        symbol: impl Into<String>,
        qty: Decimal,
        stop_price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().stop(stop_price).equity_sell(symbol, qty)
    }

    /// Equity stop-limit sell, default day order. Triggered when the
    /// market crosses `stop_price`, then becomes a limit order at
    /// `limit_price`.
    pub fn sell_stop_limit(
        symbol: impl Into<String>,
        qty: Decimal,
        stop_price: Decimal,
        limit_price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single()
            .stop_limit(stop_price, limit_price)
            .equity_sell(symbol, qty)
    }

    // --- Convenience shortcuts for common single-leg option orders ---
    //
    // `symbol` should be the Schwab option symbol (e.g.
    // `"AAPL  240315C00200000"`). Return a [`SingleOrderBuilder<Ready>`]
    // for chaining. For multi-leg option strategies (vertical, condor,
    // etc.), use [`Self::single`] with `.net_debit` / `.net_credit` and
    // chain multiple legs.

    /// Option buy-to-open at market, default day order. Opens a long
    /// option position.
    pub fn buy_to_open_market(
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().market().option_buy_to_open(symbol, qty)
    }

    /// Option buy-to-open at limit, default day order.
    pub fn buy_to_open_limit(
        symbol: impl Into<String>,
        qty: Decimal,
        price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().limit(price).option_buy_to_open(symbol, qty)
    }

    /// Option sell-to-open at market, default day order. Writes (shorts)
    /// an option.
    pub fn sell_to_open_market(
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().market().option_sell_to_open(symbol, qty)
    }

    /// Option sell-to-open at limit, default day order.
    pub fn sell_to_open_limit(
        symbol: impl Into<String>,
        qty: Decimal,
        price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().limit(price).option_sell_to_open(symbol, qty)
    }

    /// Option buy-to-close at market, default day order. Closes a
    /// previously written (short) option.
    pub fn buy_to_close_market(
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().market().option_buy_to_close(symbol, qty)
    }

    /// Option buy-to-close at limit, default day order.
    pub fn buy_to_close_limit(
        symbol: impl Into<String>,
        qty: Decimal,
        price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().limit(price).option_buy_to_close(symbol, qty)
    }

    /// Option sell-to-close at market, default day order. Closes a long
    /// option position.
    pub fn sell_to_close_market(
        symbol: impl Into<String>,
        qty: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single().market().option_sell_to_close(symbol, qty)
    }

    /// Option sell-to-close at limit, default day order.
    pub fn sell_to_close_limit(
        symbol: impl Into<String>,
        qty: Decimal,
        price: Decimal,
    ) -> SingleOrderBuilder<Ready> {
        Self::single()
            .limit(price)
            .option_sell_to_close(symbol, qty)
    }

    // --- Composite strategies ---

    /// One-cancels-other: two child orders, the first to fill cancels the
    /// other. Top-level carries only `orderStrategyType=OCO` and the two
    /// children; each child is a complete order in its own right
    /// (typically a `SINGLE`).
    ///
    /// Accepts either a finished `OrderRequest` or any
    /// [`SingleOrderBuilder<Ready>`]; the shortcuts and the explicit
    /// builder both satisfy `impl Into<OrderRequest>`.
    ///
    /// The `duration` on each child controls how long that side stays
    /// live - for a take-profit + stop-loss pair you typically want both
    /// children set to [`Duration::GoodTillCancel`] via the builder.
    ///
    /// # Examples
    ///
    /// A bracket exit: a take-profit limit paired with a stop-loss, first to
    /// fill cancels the other. Both children are good-till-cancel so neither
    /// expires at the close.
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use schwab_sdk::orders::{Duration, OrderRequest};
    ///
    /// let take_profit = OrderRequest::single()
    ///     .limit(dec!(15.27))
    ///     .equity_sell("XYZ", dec!(5))
    ///     .duration(Duration::GoodTillCancel)
    ///     .build();
    /// let stop_loss = OrderRequest::single()
    ///     .stop(dec!(11.27))
    ///     .equity_sell("XYZ", dec!(5))
    ///     .duration(Duration::GoodTillCancel)
    ///     .build();
    ///
    /// let bracket = OrderRequest::oco(take_profit, stop_loss);
    /// # let _ = bracket;
    /// ```
    pub fn oco(child_a: impl Into<OrderRequest>, child_b: impl Into<OrderRequest>) -> OrderRequest {
        OrderRequest {
            order_strategy_type: Some(OrderStrategyType::Oco),
            child_order_strategies: vec![child_a.into(), child_b.into()],
            ..OrderRequest::empty()
        }
    }

    /// First-trigger-sequence: `parent` is the order Schwab places
    /// immediately; once it fills, `child` is released. The parent's
    /// `orderStrategyType` is overwritten with `TRIGGER`.
    ///
    /// Both arguments accept any `impl Into<OrderRequest>` - the
    /// shortcuts return a [`SingleOrderBuilder<Ready>`] which is
    /// converted transparently.
    ///
    /// 1st-Trigger-OCO is the composition
    /// `OrderRequest::trigger(parent, OrderRequest::oco(profit, stop))`.
    ///
    /// # Examples
    ///
    /// Open a position, then attach a profit target and a stop once the
    /// entry fills (1st-trigger-OCO):
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use schwab_sdk::orders::{Duration, OrderRequest};
    ///
    /// let entry = OrderRequest::buy_limit("XYZ", dec!(5), dec!(14.97));
    /// let take_profit = OrderRequest::single()
    ///     .limit(dec!(15.27))
    ///     .equity_sell("XYZ", dec!(5))
    ///     .duration(Duration::GoodTillCancel)
    ///     .build();
    /// let stop_loss = OrderRequest::single()
    ///     .stop(dec!(11.27))
    ///     .equity_sell("XYZ", dec!(5))
    ///     .duration(Duration::GoodTillCancel)
    ///     .build();
    ///
    /// let order = OrderRequest::trigger(entry, OrderRequest::oco(take_profit, stop_loss));
    /// # let _ = order;
    /// ```
    pub fn trigger(
        parent: impl Into<OrderRequest>,
        child: impl Into<OrderRequest>,
    ) -> OrderRequest {
        let mut parent: OrderRequest = parent.into();
        parent.order_strategy_type = Some(OrderStrategyType::Trigger);
        parent.child_order_strategies.push(child.into());
        parent
    }
}

impl From<SingleOrderBuilder<Ready>> for OrderRequest {
    fn from(builder: SingleOrderBuilder<Ready>) -> Self {
        builder.build()
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

    /// Finish the builder and return the assembled [`OrderRequest`].
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

// --- Response -> Request conversion ---
//
// Round-trip a fetched [`Order`] back into an [`OrderRequest`] body suitable
// for `PUT /accounts/{n}/orders/{id}` (replace). Broker-assigned fields
// (`orderId`, `status`, `enteredTime`, `cancelable`, fills, lineage, etc.)
// have no place in a request and are dropped. Fields that *could* be
// represented but cannot be decoded unambiguously (e.g. a leg with no
// instrument, or an instrument with no symbol) surface as
// [`Error::OrderResponseNotRepresentable`] rather than being silently
// defaulted.

impl TryFrom<Order> for OrderRequest {
    type Error = Error;

    /// Convert a fetched [`Order`] into an [`OrderRequest`] body. Useful for
    /// constructing the body of a replace request from a previously-fetched
    /// order: take the live order, mutate the field(s) you want to change,
    /// and send it back.
    ///
    /// Broker-assigned fields (`orderId`, `status`, `enteredTime`,
    /// `cancelable`, `editable`, fills, lineage, etc.) are not part of a
    /// request body and are dropped. Child strategies (`OCO` / `TRIGGER`)
    /// are converted recursively. Fields that cannot be represented in a
    /// request (a leg missing its instrument, an instrument missing its
    /// `symbol`) surface as [`Error::OrderResponseNotRepresentable`].
    fn try_from(order: Order) -> Result<Self, Self::Error> {
        let Order {
            session,
            duration,
            order_type,
            complex_order_strategy_type,
            quantity,
            destination_link_name,
            stop_price,
            stop_price_link_basis,
            stop_price_link_type,
            stop_price_offset,
            stop_type,
            price_link_basis,
            price_link_type,
            price,
            tax_lot_method,
            order_leg_collection,
            activation_price,
            special_instruction,
            order_strategy_type,
            child_order_strategies,
            // Dropped: broker-assigned, response-only, or activity data with
            // no request counterpart.
            ..
        } = order;

        let order_leg_collection = order_leg_collection
            .into_iter()
            .map(OrderLegRequest::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let child_order_strategies = child_order_strategies
            .into_iter()
            .map(OrderRequest::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(OrderRequest {
            session,
            duration,
            order_type,
            complex_order_strategy_type,
            quantity,
            destination_link_name,
            stop_price,
            stop_price_link_basis,
            stop_price_link_type,
            stop_price_offset,
            stop_type,
            price_link_basis,
            price_link_type,
            price,
            tax_lot_method,
            order_leg_collection,
            activation_price,
            special_instruction,
            order_strategy_type,
            child_order_strategies,
        })
    }
}

impl TryFrom<OrderLegCollection> for OrderLegRequest {
    type Error = Error;

    fn try_from(leg: OrderLegCollection) -> Result<Self, Self::Error> {
        let OrderLegCollection {
            instruction,
            quantity,
            instrument,
            position_effect,
            quantity_type,
            // Dropped: response-only or carried elsewhere (the leg's asset
            // class lives on the instrument's `asset_type` instead).
            ..
        } = leg;

        let instrument = instrument
            .map(|inst| {
                let symbol = inst
                    .symbol
                    .ok_or_else(|| Error::OrderResponseNotRepresentable {
                        reason: "order leg instrument is missing `symbol`".to_string(),
                    })?;
                if let AssetType::Unknown(raw) = &inst.asset_type {
                    return Err(Error::OrderResponseNotRepresentable {
                        reason: format!("order leg instrument has unknown assetType `{raw}`"),
                    });
                }
                Ok(OrderInstrumentRequest {
                    symbol: Some(symbol),
                    asset_type: Some(inst.asset_type),
                })
            })
            .ok_or_else(|| Error::OrderResponseNotRepresentable {
                reason: "order leg is missing its instrument".to_string(),
            })
            .flatten()?;

        Ok(OrderLegRequest {
            instruction,
            quantity,
            instrument: Some(instrument),
            position_effect,
            quantity_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn pretty(value: &serde_json::Value) -> String {
        serde_json::to_string_pretty(value).unwrap()
    }

    #[test]
    fn builder_buy_market_equity_matches_schwab_example() {
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

    // --- Shortcut equivalence ---

    #[test]
    fn shortcut_buy_market_equals_explicit_builder() {
        let a = OrderRequest::buy_market("AAPL", dec!(10)).build();
        let b = OrderRequest::single()
            .market()
            .equity_buy("AAPL", dec!(10))
            .build();
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
    }

    #[test]
    fn shortcut_buy_limit_equals_explicit_builder() {
        let a = OrderRequest::buy_limit("AAPL", dec!(10), dec!(150.00)).build();
        let b = OrderRequest::single()
            .limit(dec!(150.00))
            .equity_buy("AAPL", dec!(10))
            .build();
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
    }

    #[test]
    fn shortcut_sell_stop_equals_explicit_builder() {
        let a = OrderRequest::sell_stop("AAPL", dec!(10), dec!(140.00)).build();
        let b = OrderRequest::single()
            .stop(dec!(140.00))
            .equity_sell("AAPL", dec!(10))
            .build();
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
    }

    #[test]
    fn shortcut_sell_stop_limit_equals_explicit_builder() {
        let a = OrderRequest::sell_stop_limit("AAPL", dec!(10), dec!(140.00), dec!(139.50)).build();
        let b = OrderRequest::single()
            .stop_limit(dec!(140.00), dec!(139.50))
            .equity_sell("AAPL", dec!(10))
            .build();
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
    }

    #[test]
    fn option_shortcut_buy_to_open_market_equals_explicit_builder() {
        let symbol = "AAPL  240315C00200000";
        let a = OrderRequest::buy_to_open_market(symbol, dec!(2)).build();
        let b = OrderRequest::single()
            .market()
            .option_buy_to_open(symbol, dec!(2))
            .build();
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
    }

    #[test]
    fn option_shortcuts_cover_all_four_instructions() {
        // Each option shortcut should pin the right Instruction and the
        // OPTION assetType in the resulting leg.
        let cases: [(OrderRequest, &str); 4] = [
            (
                OrderRequest::buy_to_open_limit("XYZ  240315C00500000", dec!(1), dec!(6.45))
                    .build(),
                "BUY_TO_OPEN",
            ),
            (
                OrderRequest::sell_to_open_limit("XYZ  240315C00500000", dec!(1), dec!(6.45))
                    .build(),
                "SELL_TO_OPEN",
            ),
            (
                OrderRequest::buy_to_close_limit("XYZ  240315C00500000", dec!(1), dec!(6.45))
                    .build(),
                "BUY_TO_CLOSE",
            ),
            (
                OrderRequest::sell_to_close_limit("XYZ  240315C00500000", dec!(1), dec!(6.45))
                    .build(),
                "SELL_TO_CLOSE",
            ),
        ];
        for (req, expected_instruction) in cases {
            let v = serde_json::to_value(&req).unwrap();
            let leg = &v["orderLegCollection"][0];
            assert_eq!(leg["instruction"], expected_instruction);
            assert_eq!(leg["instrument"]["assetType"], "OPTION");
            assert_eq!(v["orderStrategyType"], "SINGLE");
        }
    }

    #[test]
    fn shortcut_supports_chaining_optional_setters() {
        let req = OrderRequest::buy_limit("AAPL", dec!(10), dec!(150.00))
            .duration(Duration::GoodTillCancel)
            .session(Session::Seamless)
            .special_instruction(SpecialInstruction::AllOrNone)
            .build();
        assert_eq!(req.duration, Some(Duration::GoodTillCancel));
        assert_eq!(req.session, Some(Session::Seamless));
        assert_eq!(req.special_instruction, Some(SpecialInstruction::AllOrNone));
        // Underlying order shape is preserved.
        assert_eq!(req.order_type, Some(OrderType::Limit));
        assert_eq!(req.price, Some(dec!(150.00)));
    }

    #[test]
    fn oco_accepts_shortcut_builders_via_into() {
        // OCO takes `impl Into<OrderRequest>`, so the shortcut return
        // type (a `SingleOrderBuilder<Ready>`) flows in without
        // requiring the caller to `.build()` first.
        let oco = OrderRequest::oco(
            OrderRequest::sell_limit("XYZ", dec!(1), dec!(50)),
            OrderRequest::sell_stop("XYZ", dec!(1), dec!(40)),
        );
        let v = serde_json::to_value(&oco).unwrap();
        assert_eq!(v["orderStrategyType"], "OCO");
        assert_eq!(v["childOrderStrategies"].as_array().unwrap().len(), 2);
    }

    // --- OCO and TRIGGER strategies ---

    #[test]
    fn oco_pair_matches_schwab_example() {
        // "Sell 2 XYZ at LIMIT 45.97 or Sell 2 XYZ at STOP_LIMIT 37.03/37.00,
        // whichever fills first cancels the other. Both DAY."
        let limit_leg = OrderRequest::single()
            .limit(dec!(45.97))
            .equity_sell("XYZ", dec!(2))
            .build();
        let stop_limit_leg = OrderRequest::single()
            .stop_limit(dec!(37.03), dec!(37.00))
            .equity_sell("XYZ", dec!(2))
            .build();
        let req = OrderRequest::oco(limit_leg, stop_limit_leg);
        let actual: serde_json::Value = serde_json::to_value(&req).unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "orderStrategyType": "OCO",
                "childOrderStrategies": [
                    {
                        "orderType": "LIMIT",
                        "session": "NORMAL",
                        "price": 45.97,
                        "duration": "DAY",
                        "orderStrategyType": "SINGLE",
                        "orderLegCollection": [{
                            "instruction": "SELL",
                            "quantity": 2,
                            "instrument": { "symbol": "XYZ", "assetType": "EQUITY" }
                        }]
                    },
                    {
                        "orderType": "STOP_LIMIT",
                        "session": "NORMAL",
                        "price": 37.00,
                        "stopPrice": 37.03,
                        "duration": "DAY",
                        "orderStrategyType": "SINGLE",
                        "orderLegCollection": [{
                            "instruction": "SELL",
                            "quantity": 2,
                            "instrument": { "symbol": "XYZ", "assetType": "EQUITY" }
                        }]
                    }
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(actual, expected, "got: {}", pretty(&actual));
    }

    #[test]
    fn trigger_buy_then_sell_matches_schwab_example() {
        // "Buy 10 XYZ LIMIT 34.97. If filled, send a SELL 10 XYZ LIMIT
        // 42.03. Both DAY."
        let entry = OrderRequest::buy_limit("XYZ", dec!(10), dec!(34.97));
        let exit = OrderRequest::sell_limit("XYZ", dec!(10), dec!(42.03));
        let req = OrderRequest::trigger(entry, exit);
        let actual: serde_json::Value = serde_json::to_value(&req).unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "orderType": "LIMIT",
                "session": "NORMAL",
                "price": 34.97,
                "duration": "DAY",
                "orderStrategyType": "TRIGGER",
                "orderLegCollection": [{
                    "instruction": "BUY",
                    "quantity": 10,
                    "instrument": { "symbol": "XYZ", "assetType": "EQUITY" }
                }],
                "childOrderStrategies": [{
                    "orderType": "LIMIT",
                    "session": "NORMAL",
                    "price": 42.03,
                    "duration": "DAY",
                    "orderStrategyType": "SINGLE",
                    "orderLegCollection": [{
                        "instruction": "SELL",
                        "quantity": 10,
                        "instrument": { "symbol": "XYZ", "assetType": "EQUITY" }
                    }]
                }]
            }"#,
        )
        .unwrap();
        assert_eq!(actual, expected, "got: {}", pretty(&actual));
    }

    #[test]
    fn one_triggers_oco_matches_schwab_example() {
        // "Buy 5 XYZ LIMIT 14.97 DAY. Once filled, send an OCO of
        // (SELL 5 XYZ LIMIT 15.27 GTC) and (SELL 5 XYZ STOP 11.27 GTC)."
        let entry = OrderRequest::buy_limit("XYZ", dec!(5), dec!(14.97));
        let take_profit = OrderRequest::single()
            .limit(dec!(15.27))
            .equity_sell("XYZ", dec!(5))
            .duration(Duration::GoodTillCancel)
            .build();
        let stop_loss = OrderRequest::single()
            .stop(dec!(11.27))
            .equity_sell("XYZ", dec!(5))
            .duration(Duration::GoodTillCancel)
            .build();
        let oco = OrderRequest::oco(take_profit, stop_loss);
        let req = OrderRequest::trigger(entry, oco);
        let actual: serde_json::Value = serde_json::to_value(&req).unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "orderStrategyType": "TRIGGER",
                "session": "NORMAL",
                "duration": "DAY",
                "orderType": "LIMIT",
                "price": 14.97,
                "orderLegCollection": [{
                    "instruction": "BUY",
                    "quantity": 5,
                    "instrument": { "assetType": "EQUITY", "symbol": "XYZ" }
                }],
                "childOrderStrategies": [{
                    "orderStrategyType": "OCO",
                    "childOrderStrategies": [
                        {
                            "orderStrategyType": "SINGLE",
                            "session": "NORMAL",
                            "duration": "GOOD_TILL_CANCEL",
                            "orderType": "LIMIT",
                            "price": 15.27,
                            "orderLegCollection": [{
                                "instruction": "SELL",
                                "quantity": 5,
                                "instrument": { "assetType": "EQUITY", "symbol": "XYZ" }
                            }]
                        },
                        {
                            "orderStrategyType": "SINGLE",
                            "session": "NORMAL",
                            "duration": "GOOD_TILL_CANCEL",
                            "orderType": "STOP",
                            "stopPrice": 11.27,
                            "orderLegCollection": [{
                                "instruction": "SELL",
                                "quantity": 5,
                                "instrument": { "assetType": "EQUITY", "symbol": "XYZ" }
                            }]
                        }
                    ]
                }]
            }"#,
        )
        .unwrap();
        assert_eq!(actual, expected, "got: {}", pretty(&actual));
    }

    #[test]
    fn oco_top_level_has_no_session_or_duration() {
        // OCO is purely a composition wrapper. Schwab's documented OCO
        // example shows no top-level session/duration/orderType, only
        // `orderStrategyType` and `childOrderStrategies`.
        let a = OrderRequest::sell_limit("XYZ", dec!(1), dec!(50));
        let b = OrderRequest::sell_stop("XYZ", dec!(1), dec!(40));
        let req = OrderRequest::oco(a, b);
        let v = serde_json::to_value(&req).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("orderStrategyType"));
        assert!(obj.contains_key("childOrderStrategies"));
    }

    // --- Response -> Request round trip ---

    fn try_round_trip(req: &OrderRequest) -> OrderRequest {
        // Serialize a request, deserialize as the response shape, convert
        // back.
        let wire = serde_json::to_string(req).expect("serialize OrderRequest");
        let order: crate::orders::Order =
            serde_json::from_str(&wire).expect("deserialize as Order");
        OrderRequest::try_from(order).expect("Order -> OrderRequest")
    }

    #[test]
    fn try_from_round_trips_equity_limit_buy() {
        let req = OrderRequest::single()
            .limit(dec!(140.00))
            .equity_buy("AAPL", dec!(5))
            .duration(Duration::GoodTillCancel)
            .session(Session::Seamless)
            .special_instruction(SpecialInstruction::AllOrNone)
            .build();
        let after = try_round_trip(&req);
        assert_eq!(req, after);
    }

    #[test]
    fn try_from_round_trips_vertical_spread() {
        let req = OrderRequest::single()
            .net_debit(dec!(0.10))
            .option_buy_to_open("XYZ   240315P00045000", dec!(2))
            .option_sell_to_open("XYZ   240315P00043000", dec!(2))
            .build();
        let after = try_round_trip(&req);
        assert_eq!(req, after);
    }

    #[test]
    fn try_from_round_trips_oco_pair() {
        let limit_leg = OrderRequest::single()
            .limit(dec!(45.97))
            .equity_sell("XYZ", dec!(2))
            .build();
        let stop_limit_leg = OrderRequest::single()
            .stop_limit(dec!(37.03), dec!(37.00))
            .equity_sell("XYZ", dec!(2))
            .build();
        let req = OrderRequest::oco(limit_leg, stop_limit_leg);
        let after = try_round_trip(&req);
        assert_eq!(req, after);
    }

    #[test]
    fn try_from_round_trips_one_triggers_oco() {
        let entry = OrderRequest::buy_limit("XYZ", dec!(5), dec!(14.97));
        let take_profit = OrderRequest::single()
            .limit(dec!(15.27))
            .equity_sell("XYZ", dec!(5))
            .duration(Duration::GoodTillCancel)
            .build();
        let stop_loss = OrderRequest::single()
            .stop(dec!(11.27))
            .equity_sell("XYZ", dec!(5))
            .duration(Duration::GoodTillCancel)
            .build();
        let oco = OrderRequest::oco(take_profit, stop_loss);
        let req = OrderRequest::trigger(entry, oco);
        let after = try_round_trip(&req);
        assert_eq!(req, after);
    }

    #[test]
    fn try_from_drops_broker_assigned_fields_on_a_live_order() {
        // A live Order from a read endpoint carries broker-assigned fields
        // (orderId, status, enteredTime, fills, instrument metadata, ...).
        // The replace body must not echo any of them back; the conversion
        // drops them. Top-level fields that the response carries and that
        // the request schema also accepts (e.g. `quantity`) are preserved.
        let live: crate::orders::Order = serde_json::from_str(
            r#"{
                "orderId": 100000001,
                "accountNumber": 12345678,
                "status": "WORKING",
                "orderType": "LIMIT",
                "session": "NORMAL",
                "duration": "DAY",
                "orderStrategyType": "SINGLE",
                "quantity": 10.0,
                "filledQuantity": 0.0,
                "remainingQuantity": 10.0,
                "price": 140.00,
                "enteredTime": "2024-03-15T15:30:00.000Z",
                "cancelable": true,
                "editable": true,
                "orderLegCollection": [{
                    "orderLegType": "EQUITY",
                    "legId": 1,
                    "instruction": "BUY",
                    "quantity": 10.0,
                    "instrument": {
                        "assetType": "EQUITY",
                        "symbol": "AAPL",
                        "cusip": "037833100",
                        "instrumentId": 12345
                    }
                }]
            }"#,
        )
        .unwrap();
        let replace_body = OrderRequest::try_from(live).expect("convert live order");

        // Order shape carried through: type, prices, leg.
        assert_eq!(replace_body.order_type, Some(OrderType::Limit));
        assert_eq!(replace_body.price, Some(dec!(140.00)));
        assert_eq!(replace_body.session, Some(Session::Normal));
        assert_eq!(replace_body.duration, Some(Duration::Day));
        assert_eq!(
            replace_body.order_strategy_type,
            Some(OrderStrategyType::Single)
        );
        assert_eq!(replace_body.order_leg_collection.len(), 1);
        let leg = &replace_body.order_leg_collection[0];
        assert_eq!(leg.instruction, Some(Instruction::Buy));
        assert_eq!(leg.quantity, Some(dec!(10)));
        let inst = leg.instrument.as_ref().unwrap();
        assert_eq!(inst.symbol.as_deref(), Some("AAPL"));
        assert_eq!(inst.asset_type, Some(AssetType::Equity));

        // None of the broker-assigned fields appear in the replace body.
        let json = serde_json::to_string(&replace_body).unwrap();
        for forbidden in [
            "orderId",
            "accountNumber",
            "status",
            "enteredTime",
            "filledQuantity",
            "remainingQuantity",
            "cancelable",
            "editable",
            "cusip",
            "instrumentId",
            "legId",
        ] {
            assert!(
                !json.contains(forbidden),
                "replace body should not contain {forbidden}, got: {json}"
            );
        }
    }

    #[test]
    fn try_from_errors_when_leg_has_no_instrument() {
        let order: crate::orders::Order = serde_json::from_str(
            r#"{
                "orderId": 1,
                "orderStrategyType": "SINGLE",
                "orderType": "MARKET",
                "orderLegCollection": [{
                    "instruction": "BUY",
                    "quantity": 1
                }]
            }"#,
        )
        .unwrap();
        match OrderRequest::try_from(order) {
            Err(Error::OrderResponseNotRepresentable { reason }) => {
                assert!(reason.contains("instrument"), "unexpected reason: {reason}");
            }
            other => panic!("expected OrderResponseNotRepresentable, got {other:?}"),
        }
    }

    #[test]
    fn try_from_errors_when_instrument_has_no_symbol() {
        let order: crate::orders::Order = serde_json::from_str(
            r#"{
                "orderId": 1,
                "orderStrategyType": "SINGLE",
                "orderType": "MARKET",
                "orderLegCollection": [{
                    "instruction": "BUY",
                    "quantity": 1,
                    "instrument": { "assetType": "EQUITY" }
                }]
            }"#,
        )
        .unwrap();
        match OrderRequest::try_from(order) {
            Err(Error::OrderResponseNotRepresentable { reason }) => {
                assert!(reason.contains("symbol"), "unexpected reason: {reason}");
            }
            other => panic!("expected OrderResponseNotRepresentable, got {other:?}"),
        }
    }

    #[test]
    fn try_from_errors_when_asset_type_is_unknown() {
        // Schwab may asset types over time; `string_enum!` decodes any
        // unrecognized value to `AssetType::Unknown(_)`. The request side
        // cannot safely send those back to a different endpoint, so the
        // conversion refuses rather than guessing.
        let order: crate::orders::Order = serde_json::from_str(
            r#"{
                "orderId": 1,
                "orderStrategyType": "SINGLE",
                "orderType": "MARKET",
                "orderLegCollection": [{
                    "instruction": "BUY",
                    "quantity": 1,
                    "instrument": { "assetType": "NEW_ASSET_CLASS", "symbol": "X" }
                }]
            }"#,
        )
        .unwrap();
        match OrderRequest::try_from(order) {
            Err(Error::OrderResponseNotRepresentable { reason }) => {
                assert!(
                    reason.contains("NEW_ASSET_CLASS"),
                    "unexpected reason: {reason}"
                );
            }
            other => panic!("expected OrderResponseNotRepresentable, got {other:?}"),
        }
    }

    #[test]
    fn try_from_error_is_not_retryable() {
        let err = Error::OrderResponseNotRepresentable {
            reason: "leg missing instrument".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.retry_after(), None);
    }
}
