//! Response shapes for the `/orders` and `/accounts/{n}/orders*` GET
//! endpoints. The construction-side types live in
//! [`super::request`](crate::orders::request).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use crate::accounts::AccountsInstrument;
use crate::orders::enums::*;

/// One order, as returned by the read endpoints. Schwab marks almost no
/// field as required, so everything outside the discriminator-bearing
/// enums is `Option`.
///
/// The OpenAPI spec types `accountNumber` and `orderId` as plain `int64`
/// here (in contrast to the string-typed account number on
/// `securitiesAccount`). The fields are kept as numeric here to match.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
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
    fn empty_collections_default_to_empty_vecs() {
        let json = r#"{"orderId": 1}"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert!(order.order_leg_collection.is_empty());
        assert!(order.order_activity_collection.is_empty());
        assert!(order.child_order_strategies.is_empty());
        assert!(order.replacing_order_collection.is_empty());
    }
}
