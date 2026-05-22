//! String-valued enums shared across the orders request/response shapes.
//!
//! Every variant has a `Unknown(String)` catch-all so wire values added by
//! Schwab after this crate was published deserialize cleanly. The
//! `string_enum!` macro lives in [`crate::macros`].

use serde::{Deserialize, Serialize};

use crate::macros::string_enum;

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
}
