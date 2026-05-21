//! Request shapes and the typestate builder for constructing them.
//!
//! [`OrderRequest`] is the body of `POST /accounts/{n}/orders` (place) and
//! `PUT /accounts/{n}/orders/{id}` (replace). Construct via
//! [`OrderRequest::single`] for the typed-state builder, or by populating
//! fields directly for shapes the builder does not yet cover.

use std::marker::PhantomData;

use rust_decimal::Decimal;
use serde::Serialize;

use crate::api::accounts::AssetType;
use crate::api::orders::enums::{
    ComplexOrderStrategyType, Duration, Instruction, OrderStrategyType, OrderType, PositionEffect,
    PriceLinkBasis, PriceLinkType, QuantityType, Session, SpecialInstruction, StopPriceLinkBasis,
    StopPriceLinkType, StopType, TaxLotMethod,
};

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
/// via [`OrderRequest::single`] (typestate builder) or by populating
/// fields directly.
///
/// Response-only fields (`status`, `filledQuantity`, `enteredTime`,
/// `tag`, `requestedDestination`, etc.) are not present here; they live
/// on [`Order`](crate::api::orders::Order) instead.
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
/// settable. Uses the typed [`AssetType`] from
/// [`crate::api::accounts`].
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
}
