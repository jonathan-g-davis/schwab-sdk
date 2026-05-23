//! Response shape for `POST /accounts/{accountNumber}/previewOrder`.
//!
//! Preview returns Schwab's view of what would happen if an
//! [`OrderRequest`](crate::orders::OrderRequest) were submitted:
//! computed commissions, fees, projected buying-power impact, and any
//! validation alerts / rejects the order would trigger.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::serde::float_option as decimal_opt;
use serde::{Deserialize, Serialize};

use crate::accounts::AssetType;
use crate::macros::string_enum;
use crate::orders::enums::{
    ApiOrderStatus, ComplexOrderStrategyType, Duration, Instruction, OrderStrategyType, OrderType,
    Session,
};

/// Top-level response body of `previewOrder`.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct PreviewOrder {
    #[serde(default, rename = "orderId")]
    pub order_id: Option<i64>,
    #[serde(default, rename = "orderStrategy")]
    pub order_strategy: Option<OrderStrategy>,
    #[serde(default, rename = "orderValidationResult")]
    pub order_validation_result: Option<OrderValidationResult>,
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
    #[serde(default, rename = "accountNumber")]
    pub account_number: Option<String>,
    #[serde(default, rename = "advancedOrderType")]
    pub advanced_order_type: Option<AdvancedOrderType>,
    #[serde(default, rename = "closeTime")]
    pub close_time: Option<DateTime<Utc>>,
    #[serde(default, rename = "enteredTime")]
    pub entered_time: Option<DateTime<Utc>>,
    #[serde(default, rename = "orderBalance")]
    pub order_balance: Option<OrderBalance>,
    #[serde(default, rename = "orderStrategyType")]
    pub order_strategy_type: Option<OrderStrategyType>,
    #[serde(default, with = "decimal_opt", rename = "orderVersion")]
    pub order_version: Option<Decimal>,
    #[serde(default)]
    pub session: Option<Session>,
    #[serde(default)]
    pub status: Option<ApiOrderStatus>,
    #[serde(default, rename = "allOrNone")]
    pub all_or_none: Option<bool>,
    #[serde(default)]
    pub discretionary: Option<bool>,
    #[serde(default)]
    pub duration: Option<Duration>,
    #[serde(default, with = "decimal_opt", rename = "filledQuantity")]
    pub filled_quantity: Option<Decimal>,
    #[serde(default, rename = "orderType")]
    pub order_type: Option<OrderType>,
    #[serde(default, with = "decimal_opt", rename = "orderValue")]
    pub order_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub price: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "remainingQuantity")]
    pub remaining_quantity: Option<Decimal>,
    #[serde(default, rename = "sellNonMarginableFirst")]
    pub sell_non_marginable_first: Option<bool>,
    #[serde(default, rename = "settlementInstruction")]
    pub settlement_instruction: Option<SettlementInstruction>,
    #[serde(default)]
    pub strategy: Option<ComplexOrderStrategyType>,
    #[serde(default, rename = "amountIndicator")]
    pub amount_indicator: Option<AmountIndicator>,
    #[serde(default, rename = "orderLegs")]
    pub order_legs: Vec<OrderLeg>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderBalance {
    #[serde(default, with = "decimal_opt", rename = "orderValue")]
    pub order_value: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "projectedAvailableFund")]
    pub projected_available_fund: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "projectedBuyingPower")]
    pub projected_buying_power: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "projectedCommission")]
    pub projected_commission: Option<Decimal>,
}

/// Per-leg preview entry. Distinct from the response-side
/// [`OrderLegCollection`](crate::orders::OrderLegCollection): preview
/// adds market quotes (bid/ask/last/mark) and a projected commission.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderLeg {
    #[serde(default, with = "decimal_opt", rename = "askPrice")]
    pub ask_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "bidPrice")]
    pub bid_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "lastPrice")]
    pub last_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "markPrice")]
    pub mark_price: Option<Decimal>,
    #[serde(default, with = "decimal_opt", rename = "projectedCommission")]
    pub projected_commission: Option<Decimal>,
    #[serde(default, with = "decimal_opt")]
    pub quantity: Option<Decimal>,
    #[serde(default, rename = "finalSymbol")]
    pub final_symbol: Option<String>,
    #[serde(default, with = "decimal_opt", rename = "legId")]
    pub leg_id: Option<Decimal>,
    #[serde(default, rename = "assetType")]
    pub asset_type: Option<AssetType>,
    #[serde(default)]
    pub instruction: Option<Instruction>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderValidationResult {
    #[serde(default)]
    pub alerts: Vec<OrderValidationDetail>,
    #[serde(default)]
    pub accepts: Vec<OrderValidationDetail>,
    #[serde(default)]
    pub rejects: Vec<OrderValidationDetail>,
    #[serde(default)]
    pub reviews: Vec<OrderValidationDetail>,
    #[serde(default)]
    pub warns: Vec<OrderValidationDetail>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct OrderValidationDetail {
    #[serde(default, rename = "validationRuleName")]
    pub validation_rule_name: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default, rename = "activityMessage")]
    pub activity_message: Option<String>,
    #[serde(default, rename = "originalSeverity")]
    pub original_severity: Option<ApiRuleAction>,
    #[serde(default, rename = "overrideName")]
    pub override_name: Option<String>,
    #[serde(default, rename = "overrideSeverity")]
    pub override_severity: Option<ApiRuleAction>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct CommissionAndFee {
    #[serde(default)]
    pub commission: Option<Commission>,
    #[serde(default)]
    pub fee: Option<Fees>,
    /// Some Schwab responses include a `trueCommission` shadow for
    /// reconciliation; preserved verbatim here.
    #[serde(default, rename = "trueCommission")]
    pub true_commission: Option<Commission>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Commission {
    #[serde(default, rename = "commissionLegs")]
    pub commission_legs: Vec<CommissionLeg>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct CommissionLeg {
    #[serde(default, rename = "commissionValues")]
    pub commission_values: Vec<CommissionValue>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct CommissionValue {
    #[serde(default, with = "decimal_opt")]
    pub value: Option<Decimal>,
    #[serde(default, rename = "type")]
    pub fee_type: Option<FeeType>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct Fees {
    #[serde(default, rename = "feeLegs")]
    pub fee_legs: Vec<FeeLeg>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct FeeLeg {
    #[serde(default, rename = "feeValues")]
    pub fee_values: Vec<FeeValue>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct FeeValue {
    #[serde(default, with = "decimal_opt")]
    pub value: Option<Decimal>,
    #[serde(default, rename = "type")]
    pub fee_type: Option<FeeType>,
}

// --- Enums ---

string_enum! {
    AdvancedOrderType {
        None = "NONE",
        Oto = "OTO",
        Oco = "OCO",
        Otoco = "OTOCO",
        Ot2oco = "OT2OCO",
        Ot3oco = "OT3OCO",
        BlastAll = "BLAST_ALL",
        Ota = "OTA",
        Pair = "PAIR",
    }
}

string_enum! {
    ApiRuleAction {
        Accept = "ACCEPT",
        Alert = "ALERT",
        Reject = "REJECT",
        Review = "REVIEW",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    AmountIndicator {
        Dollars = "DOLLARS",
        Shares = "SHARES",
        AllShares = "ALL_SHARES",
        Percentage = "PERCENTAGE",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    SettlementInstruction {
        Regular = "REGULAR",
        Cash = "CASH",
        NextDay = "NEXT_DAY",
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Fee type discriminator used inside the preview's commission and fee
    /// breakdowns. Distinct from
    /// [`crate::transactions::FeeType`]; the preview spec lists more
    /// variants (futures-specific fees, taxes) than the transactions spec.
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
}
