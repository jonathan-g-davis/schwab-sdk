//! Response shape for `POST /accounts/{accountNumber}/previewOrder`.
//!
//! Preview returns Schwab's view of what would happen if an
//! [`OrderRequest`](crate::orders::OrderRequest) were submitted:
//! computed commissions, fees, projected buying-power impact, and any
//! validation alerts / rejects the order would trigger.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::Deserialize;

use crate::accounts::AssetType;
use crate::macros::string_enum;
use crate::orders::enums::{
    ApiOrderStatus, ComplexOrderStrategyType, Duration, Instruction, OrderStrategyType, OrderType,
    Session,
};
use crate::secrets::AccountNumber;

/// Top-level response body of `previewOrder`.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct PreviewOrder {
    /// Placeholder order id Schwab assigns to the preview row.
    /// Not a placeable id; do not pass it back into `replace` or `cancel`.
    #[serde(default, rename = "orderId")]
    pub order_id: Option<i64>,
    /// Order as Schwab projects it would be recorded.
    #[serde(default, rename = "orderStrategy")]
    pub order_strategy: Option<OrderStrategy>,
    /// Per-rule validation outcomes. Inspect [`OrderValidationResult::rejects`]
    /// before treating a 200 response as approval.
    #[serde(default, rename = "orderValidationResult")]
    pub order_validation_result: Option<OrderValidationResult>,
    /// Projected commissions and fees.
    #[serde(default, rename = "commissionAndFee")]
    pub commission_and_fee: Option<CommissionAndFee>,
}

/// The "order as Schwab would record it" projection inside a preview.
/// Closely shadows [`Order`](crate::orders::Order) but with
/// preview-specific fields (`orderBalance`, `advancedOrderType`,
/// `orderVersion`, etc.).
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderStrategy {
    /// Plain account number that owns the projected order.
    #[serde(default, rename = "accountNumber")]
    pub account_number: Option<AccountNumber>,
    /// Advanced-order shape (OTO / OCO / OTOCO / ...).
    #[serde(default, rename = "advancedOrderType")]
    pub advanced_order_type: Option<AdvancedOrderType>,
    /// Time the order would close (terminal state).
    #[serde(default, rename = "closeTime")]
    pub close_time: Option<DateTime<Utc>>,
    /// Time Schwab would record the order.
    #[serde(default, rename = "enteredTime")]
    pub entered_time: Option<DateTime<Utc>>,
    /// Projected impact on account balances.
    #[serde(default, rename = "orderBalance")]
    pub order_balance: Option<OrderBalance>,
    /// Top-level structure of the order envelope.
    #[serde(default, rename = "orderStrategyType")]
    pub order_strategy_type: Option<OrderStrategyType>,
    /// Order-version sequence number Schwab assigns on each replace.
    #[serde(default, with = "decimal_opt", rename = "orderVersion")]
    pub order_version: Option<Decimal>,
    /// Trading session.
    #[serde(default)]
    pub session: Option<Session>,
    /// Lifecycle status of the projected order.
    #[serde(default)]
    pub status: Option<ApiOrderStatus>,
    /// `true` if the order is all-or-none.
    #[serde(default, rename = "allOrNone")]
    pub all_or_none: Option<bool>,
    /// `true` if the order is discretionary (broker may improve price).
    #[serde(default)]
    pub discretionary: Option<bool>,
    /// Time-in-force.
    #[serde(default)]
    pub duration: Option<Duration>,
    /// Projected filled quantity.
    #[serde(default, with = "decimal_opt", rename = "filledQuantity")]
    pub filled_quantity: Option<Decimal>,
    /// Order type (market / limit / stop / ...).
    #[serde(default, rename = "orderType")]
    pub order_type: Option<OrderType>,
    /// Total notional value of the order, USD.
    #[serde(default, with = "decimal_opt", rename = "orderValue")]
    pub order_value: Option<Decimal>,
    /// Limit price, USD.
    #[serde(default, with = "decimal_opt")]
    pub price: Option<Decimal>,
    /// Order quantity.
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    /// Projected remaining quantity after fill.
    #[serde(default, with = "decimal_opt", rename = "remainingQuantity")]
    pub remaining_quantity: Option<Decimal>,
    /// `true` if Schwab would prefer to sell non-marginable shares first.
    #[serde(default, rename = "sellNonMarginableFirst")]
    pub sell_non_marginable_first: Option<bool>,
    /// Settlement timing instruction.
    #[serde(default, rename = "settlementInstruction")]
    pub settlement_instruction: Option<SettlementInstruction>,
    /// Multi-leg option strategy shape.
    #[serde(default)]
    pub strategy: Option<ComplexOrderStrategyType>,
    /// How the leg's quantity is denominated (shares / dollars / percentage).
    #[serde(default, rename = "amountIndicator")]
    pub amount_indicator: Option<AmountIndicator>,
    /// Per-leg projected detail (quote, commission, etc.).
    #[serde(default, rename = "orderLegs")]
    pub order_legs: Vec<OrderLeg>,
}

/// Projected balance impact of the order, USD.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderBalance {
    /// Notional value of the order.
    #[serde(default, with = "decimal_opt", rename = "orderValue")]
    pub order_value: Option<Decimal>,
    /// Available funds after the order would settle.
    #[serde(default, with = "decimal_opt", rename = "projectedAvailableFund")]
    pub projected_available_fund: Option<Decimal>,
    /// Buying power after the order would settle.
    #[serde(default, with = "decimal_opt", rename = "projectedBuyingPower")]
    pub projected_buying_power: Option<Decimal>,
    /// Projected commission charged for the order.
    #[serde(default, with = "decimal_opt", rename = "projectedCommission")]
    pub projected_commission: Option<Decimal>,
}

/// Per-leg preview entry. Distinct from the response-side
/// [`OrderLegCollection`](crate::orders::OrderLegCollection): preview
/// adds market quotes (bid/ask/last/mark) and a projected commission.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderLeg {
    /// Best ask at preview time, USD.
    #[serde(default, with = "decimal_opt", rename = "askPrice")]
    pub ask_price: Option<Decimal>,
    /// Best bid at preview time, USD.
    #[serde(default, with = "decimal_opt", rename = "bidPrice")]
    pub bid_price: Option<Decimal>,
    /// Last trade at preview time, USD.
    #[serde(default, with = "decimal_opt", rename = "lastPrice")]
    pub last_price: Option<Decimal>,
    /// Mark price at preview time, USD.
    #[serde(default, with = "decimal_opt", rename = "markPrice")]
    pub mark_price: Option<Decimal>,
    /// Projected commission for this leg, USD.
    #[serde(default, with = "decimal_opt", rename = "projectedCommission")]
    pub projected_commission: Option<Decimal>,
    /// Leg quantity.
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    /// Symbol Schwab resolved to (after corporate-action / root-symbol fixup).
    #[serde(default, rename = "finalSymbol")]
    pub final_symbol: Option<String>,
    /// Schwab-assigned leg id within the preview order.
    #[serde(default, rename = "legId")]
    pub leg_id: Option<i64>,
    /// Asset class of the leg.
    #[serde(default, rename = "assetType")]
    pub asset_type: Option<AssetType>,
    /// Side / intent (buy / sell / buy-to-cover / ...).
    #[serde(default)]
    pub instruction: Option<Instruction>,
}

/// Per-rule outcome of the preview's validation pass.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderValidationResult {
    /// Soft-warning rules.
    #[serde(default)]
    pub alerts: Vec<OrderValidationDetail>,
    /// Rules that explicitly accept the order.
    #[serde(default)]
    pub accepts: Vec<OrderValidationDetail>,
    /// Rules that reject the order; non-empty means the order would not
    /// be placeable as-is.
    #[serde(default)]
    pub rejects: Vec<OrderValidationDetail>,
    /// Rules flagging the order for manual review.
    #[serde(default)]
    pub reviews: Vec<OrderValidationDetail>,
    /// Hard-warning rules.
    #[serde(default)]
    pub warns: Vec<OrderValidationDetail>,
}

/// Detail for one rule outcome inside an [`OrderValidationResult`].
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderValidationDetail {
    /// Schwab-internal rule name (e.g. `"BUYING_POWER_CHECK"`).
    #[serde(default, rename = "validationRuleName")]
    pub validation_rule_name: Option<String>,
    /// Human-readable rule message.
    #[serde(default)]
    pub message: Option<String>,
    /// Secondary message tied to the activity that triggered the rule.
    #[serde(default, rename = "activityMessage")]
    pub activity_message: Option<String>,
    /// Rule severity as Schwab originally classified it.
    #[serde(default, rename = "originalSeverity")]
    pub original_severity: Option<ApiRuleAction>,
    /// Override identifier when Schwab downgraded the rule.
    #[serde(default, rename = "overrideName")]
    pub override_name: Option<String>,
    /// Severity after an override was applied.
    #[serde(default, rename = "overrideSeverity")]
    pub override_severity: Option<ApiRuleAction>,
}

/// Combined commission and fee projection for the preview.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct CommissionAndFee {
    /// Projected commissions, broken down by leg.
    #[serde(default)]
    pub commission: Option<Commission>,
    /// Projected regulatory and venue fees, broken down by leg.
    #[serde(default)]
    pub fee: Option<Fees>,
    /// Some Schwab responses include a `trueCommission` shadow for
    /// reconciliation; preserved verbatim here.
    #[serde(default, rename = "trueCommission")]
    pub true_commission: Option<Commission>,
}

/// Per-leg commission projection.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Commission {
    /// One entry per order leg.
    #[serde(default, rename = "commissionLegs")]
    pub commission_legs: Vec<CommissionLeg>,
}

/// One leg's projected commission values.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct CommissionLeg {
    /// One entry per commission component (e.g. base commission, surcharge).
    #[serde(default, rename = "commissionValues")]
    pub commission_values: Vec<CommissionValue>,
}

/// A single commission component within a [`CommissionLeg`].
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct CommissionValue {
    /// Value of this commission component, USD.
    #[serde(default, with = "decimal_opt")]
    pub value: Option<Decimal>,
    /// Classification (commission / fee variant) for this value.
    #[serde(default, rename = "type")]
    pub fee_type: Option<FeeType>,
}

/// Per-leg fee projection (regulatory and venue fees).
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Fees {
    /// One entry per order leg.
    #[serde(default, rename = "feeLegs")]
    pub fee_legs: Vec<FeeLeg>,
}

/// One leg's projected fee values.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct FeeLeg {
    /// One entry per fee component (SEC, TAF, OPT_REG, ...).
    #[serde(default, rename = "feeValues")]
    pub fee_values: Vec<FeeValue>,
}

/// A single fee component within a [`FeeLeg`].
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct FeeValue {
    /// Value of this fee component, USD.
    #[serde(default, with = "decimal_opt")]
    pub value: Option<Decimal>,
    /// Fee classification.
    #[serde(default, rename = "type")]
    pub fee_type: Option<FeeType>,
}

// --- Enums ---

string_enum! {
    /// Multi-step order structure (OTO / OCO / OTOCO / ...).
    AdvancedOrderType {
        /// Not an advanced order.
        None = "NONE",
        /// One-triggers-other.
        Oto = "OTO",
        /// One-cancels-other.
        Oco = "OCO",
        /// One-triggers-one-cancels-other.
        Otoco = "OTOCO",
        /// One-triggers-two-cancels-other.
        Ot2oco = "OT2OCO",
        /// One-triggers-three-cancels-other.
        Ot3oco = "OT3OCO",
        /// Blast-all (send to multiple venues).
        BlastAll = "BLAST_ALL",
        /// One-triggers-another.
        Ota = "OTA",
        /// Pair-trade.
        Pair = "PAIR",
    }
}

string_enum! {
    /// Severity Schwab assigns to a validation rule result.
    ApiRuleAction {
        /// Rule explicitly accepted the order.
        Accept = "ACCEPT",
        /// Soft alert; order would still be placeable.
        Alert = "ALERT",
        /// Hard reject; order would not be placed.
        Reject = "REJECT",
        /// Flagged for manual review.
        Review = "REVIEW",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// How an order leg's quantity is denominated.
    AmountIndicator {
        /// Dollar-denominated (fractional shares).
        Dollars = "DOLLARS",
        /// Whole-share count.
        Shares = "SHARES",
        /// Close out the entire existing position.
        AllShares = "ALL_SHARES",
        /// Percentage of position.
        Percentage = "PERCENTAGE",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Settlement timing preference for an order.
    SettlementInstruction {
        /// Standard T+2 (equities) / T+1 (options) settlement.
        Regular = "REGULAR",
        /// Same-day cash settlement.
        Cash = "CASH",
        /// Next-day settlement.
        NextDay = "NEXT_DAY",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Fee type discriminator used inside the preview's commission and fee
    /// breakdowns. Distinct from
    /// [`crate::transactions::FeeType`]; the preview spec lists more
    /// variants (futures-specific fees, taxes) than the transactions spec.
    FeeType {
        /// Broker commission.
        Commission = "COMMISSION",
        /// SEC Section-31 fee.
        SecFee = "SEC_FEE",
        /// Securities Transaction Reporting (STR) fee.
        StrFee = "STR_FEE",
        /// Regulatory R-fee.
        RFee = "R_FEE",
        /// Contingent deferred sales charge.
        CdscFee = "CDSC_FEE",
        /// Options Regulatory Fee.
        OptRegFee = "OPT_REG_FEE",
        /// Additional miscellaneous charge.
        AdditionalFee = "ADDITIONAL_FEE",
        /// Catch-all miscellaneous fee.
        MiscellaneousFee = "MISCELLANEOUS_FEE",
        /// Financial Transaction Tax.
        Ftt = "FTT",
        /// Futures clearing fee.
        FuturesClearingFee = "FUTURES_CLEARING_FEE",
        /// Futures desk-office fee.
        FuturesDeskOfficeFee = "FUTURES_DESK_OFFICE_FEE",
        /// Futures exchange fee.
        FuturesExchangeFee = "FUTURES_EXCHANGE_FEE",
        /// CME Globex venue fee.
        FuturesGlobexFee = "FUTURES_GLOBEX_FEE",
        /// National Futures Association fee.
        FuturesNfaFee = "FUTURES_NFA_FEE",
        /// Futures pit-brokerage fee.
        FuturesPitBrokerageFee = "FUTURES_PIT_BROKERAGE_FEE",
        /// Futures transaction fee.
        FuturesTransactionFee = "FUTURES_TRANSACTION_FEE",
        /// Reduced commission applied to low-proceed trades.
        LowProceedsCommission = "LOW_PROCEEDS_COMMISSION",
        /// Base charge.
        BaseCharge = "BASE_CHARGE",
        /// General charge.
        GeneralCharge = "GENERAL_CHARGE",
        /// Australian GST fee.
        GstFee = "GST_FEE",
        /// Trading Activity Fee (FINRA).
        TafFee = "TAF_FEE",
        /// OCC index-option processing fee.
        IndexOptionFee = "INDEX_OPTION_FEE",
        /// TEFRA backup withholding.
        TefraTax = "TEFRA_TAX",
        /// State-level tax.
        StateTax = "STATE_TAX",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn preview_with_validation_and_fees_parses() {
        let json = r#"{
            "orderId": 0,
            "orderStrategy": {
                "accountNumber": "12345678",
                "advancedOrderType": "NONE",
                "orderStrategyType": "SINGLE",
                "session": "NORMAL",
                "status": "AWAITING_PARENT_ORDER",
                "duration": "DAY",
                "orderType": "LIMIT",
                "price": 145.32,
                "quantity": 10,
                "orderValue": 1453.20,
                "orderBalance": {
                    "orderValue": 1453.20,
                    "projectedAvailableFund": 8500.00,
                    "projectedBuyingPower": 17000.00,
                    "projectedCommission": 0.00
                },
                "orderLegs": [{
                    "askPrice": 145.35,
                    "bidPrice": 145.30,
                    "lastPrice": 145.32,
                    "markPrice": 145.32,
                    "projectedCommission": 0.00,
                    "quantity": 10,
                    "finalSymbol": "AAPL",
                    "assetType": "EQUITY",
                    "instruction": "BUY"
                }]
            },
            "orderValidationResult": {
                "alerts": [],
                "accepts": [{
                    "validationRuleName": "BUYING_POWER_CHECK",
                    "message": "Sufficient buying power"
                }],
                "rejects": [],
                "reviews": [],
                "warns": []
            },
            "commissionAndFee": {
                "commission": {
                    "commissionLegs": [{
                        "commissionValues": [{
                            "value": 0.00,
                            "type": "COMMISSION"
                        }]
                    }]
                },
                "fee": {
                    "feeLegs": [{
                        "feeValues": [{
                            "value": 0.02,
                            "type": "SEC_FEE"
                        }]
                    }]
                }
            }
        }"#;
        let preview: PreviewOrder = serde_json::from_str(json).unwrap();
        let strategy = preview.order_strategy.as_ref().unwrap();
        assert_eq!(strategy.order_type, Some(OrderType::Limit));
        assert_eq!(strategy.price, Some(dec!(145.32)));
        assert_eq!(strategy.quantity, Some(dec!(10)));
        assert_eq!(strategy.order_value, Some(dec!(1453.20)));
        assert_eq!(strategy.advanced_order_type, Some(AdvancedOrderType::None));

        let balance = strategy.order_balance.as_ref().unwrap();
        assert_eq!(balance.projected_available_fund, Some(dec!(8500.00)));

        assert_eq!(strategy.order_legs.len(), 1);
        let leg = &strategy.order_legs[0];
        assert_eq!(leg.bid_price, Some(dec!(145.30)));
        assert_eq!(leg.ask_price, Some(dec!(145.35)));
        assert_eq!(leg.instruction, Some(Instruction::Buy));

        let validation = preview.order_validation_result.as_ref().unwrap();
        assert!(validation.alerts.is_empty());
        assert!(validation.rejects.is_empty());
        assert_eq!(validation.accepts.len(), 1);
        assert_eq!(
            validation.accepts[0].validation_rule_name.as_deref(),
            Some("BUYING_POWER_CHECK")
        );

        let fees = preview.commission_and_fee.as_ref().unwrap();
        let commission_value =
            &fees.commission.as_ref().unwrap().commission_legs[0].commission_values[0];
        assert_eq!(commission_value.value, Some(dec!(0.00)));
        assert_eq!(commission_value.fee_type, Some(FeeType::Commission));
        let fee_value = &fees.fee.as_ref().unwrap().fee_legs[0].fee_values[0];
        assert_eq!(fee_value.value, Some(dec!(0.02)));
        assert_eq!(fee_value.fee_type, Some(FeeType::SecFee));
    }

    #[test]
    fn preview_with_reject_parses() {
        let json = r#"{
            "orderValidationResult": {
                "rejects": [{
                    "validationRuleName": "NEGATIVE_BUYING_POWER",
                    "message": "Insufficient buying power",
                    "originalSeverity": "REJECT"
                }]
            }
        }"#;
        let preview: PreviewOrder = serde_json::from_str(json).unwrap();
        let validation = preview.order_validation_result.unwrap();
        assert_eq!(validation.rejects.len(), 1);
        assert_eq!(
            validation.rejects[0].original_severity,
            Some(ApiRuleAction::Reject)
        );
    }

    #[test]
    fn empty_preview_object_parses() {
        let preview: PreviewOrder = serde_json::from_str("{}").unwrap();
        assert!(preview.order_id.is_none());
        assert!(preview.order_strategy.is_none());
    }

    #[test]
    fn unknown_advanced_order_type_preserves_raw_string() {
        let parsed: AdvancedOrderType = serde_json::from_str(r#""OT4OCO""#).unwrap();
        assert!(matches!(parsed, AdvancedOrderType::Unknown(ref s) if s == "OT4OCO"));
    }

    #[test]
    fn preview_fee_type_round_trips_futures_specific_variants() {
        for raw in [
            "FTT",
            "FUTURES_CLEARING_FEE",
            "FUTURES_GLOBEX_FEE",
            "TEFRA_TAX",
            "STATE_TAX",
        ] {
            let json = format!(r#""{raw}""#);
            let parsed: FeeType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn account_number_in_order_strategy_redacts_on_debug() {
        let json = r#"{"accountNumber": "12345678"}"#;
        let strategy: OrderStrategy = serde_json::from_str(json).unwrap();
        let debug = format!("{:?}", strategy);
        assert!(
            !debug.contains("12345678"),
            "account number leaked through Debug: {debug}"
        );
    }
}
