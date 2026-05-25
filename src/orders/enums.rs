//! String-valued enums shared across the orders request/response shapes.
//!
//! Every variant has a `Unknown(String)` catch-all so wire values added by
//! Schwab after this crate was published deserialize cleanly. The
//! `string_enum!` macro lives in [`crate::macros`].

use crate::macros::string_enum;

string_enum! {
    /// Which trading session the order is valid in.
    Session {
        /// Regular session.
        Normal = "NORMAL",
        /// Pre-market session.
        Am = "AM",
        /// Post-market session.
        Pm = "PM",
        /// All sessions; Schwab routes wherever the order is fillable.
        Seamless = "SEAMLESS",
    }
}

string_enum! {
    /// Time-in-force for an order.
    Duration {
        /// Expires at the end of the regular session.
        Day = "DAY",
        /// Stays open until filled or explicitly cancelled.
        GoodTillCancel = "GOOD_TILL_CANCEL",
        /// Fill the entire order immediately or cancel it.
        FillOrKill = "FILL_OR_KILL",
        /// Fill whatever can fill immediately; cancel the rest.
        ImmediateOrCancel = "IMMEDIATE_OR_CANCEL",
        /// Expires at the end of the trading week.
        EndOfWeek = "END_OF_WEEK",
        /// Expires at the end of the trading month.
        EndOfMonth = "END_OF_MONTH",
        /// Expires at the end of the next trading month.
        NextEndOfMonth = "NEXT_END_OF_MONTH",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// How the order's fill price is determined.
    OrderType {
        /// Fill at the best available market price.
        Market = "MARKET",
        /// Fill at the specified price or better.
        Limit = "LIMIT",
        /// Becomes a market order once the stop is touched.
        Stop = "STOP",
        /// Becomes a limit order once the stop is touched.
        StopLimit = "STOP_LIMIT",
        /// Stop that follows the market at a fixed offset.
        TrailingStop = "TRAILING_STOP",
        /// Cabinet (zero-premium) options trade.
        Cabinet = "CABINET",
        /// Limit order priced away from the inside market.
        NonMarketable = "NON_MARKETABLE",
        /// Market order executed in the closing auction.
        MarketOnClose = "MARKET_ON_CLOSE",
        /// Exercise of a long option.
        Exercise = "EXERCISE",
        /// Trailing stop that becomes a limit order once triggered.
        TrailingStopLimit = "TRAILING_STOP_LIMIT",
        /// Multi-leg order with a net debit price.
        NetDebit = "NET_DEBIT",
        /// Multi-leg order with a net credit price.
        NetCredit = "NET_CREDIT",
        /// Multi-leg order with a net price of zero.
        NetZero = "NET_ZERO",
        /// Limit order executed in the closing auction.
        LimitOnClose = "LIMIT_ON_CLOSE",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// Top-level structure of an order envelope.
    OrderStrategyType {
        /// Single-leg order.
        Single = "SINGLE",
        /// Cancel an existing order.
        Cancel = "CANCEL",
        /// Recall an existing order.
        Recall = "RECALL",
        /// Pair-trade strategy.
        Pair = "PAIR",
        /// Flatten an account or position.
        Flatten = "FLATTEN",
        /// Two-day swap strategy.
        TwoDaySwap = "TWO_DAY_SWAP",
        /// Blast-all (send to multiple venues).
        BlastAll = "BLAST_ALL",
        /// One-cancels-other.
        Oco = "OCO",
        /// One-triggers-other.
        Trigger = "TRIGGER",
    }
}

string_enum! {
    /// Multi-leg option strategy shape.
    ComplexOrderStrategyType {
        /// Not a complex strategy.
        None = "NONE",
        /// Covered call / covered put.
        Covered = "COVERED",
        /// Vertical spread.
        Vertical = "VERTICAL",
        /// Back-ratio spread.
        BackRatio = "BACK_RATIO",
        /// Calendar (horizontal) spread.
        Calendar = "CALENDAR",
        /// Diagonal spread.
        Diagonal = "DIAGONAL",
        /// Straddle.
        Straddle = "STRADDLE",
        /// Strangle.
        Strangle = "STRANGLE",
        /// Synthetic collar.
        CollarSynthetic = "COLLAR_SYNTHETIC",
        /// Butterfly.
        Butterfly = "BUTTERFLY",
        /// Condor.
        Condor = "CONDOR",
        /// Iron condor.
        IronCondor = "IRON_CONDOR",
        /// Vertical roll.
        VerticalRoll = "VERTICAL_ROLL",
        /// Collar paired with the underlying stock.
        CollarWithStock = "COLLAR_WITH_STOCK",
        /// Double diagonal.
        DoubleDiagonal = "DOUBLE_DIAGONAL",
        /// Unbalanced butterfly.
        UnbalancedButterfly = "UNBALANCED_BUTTERFLY",
        /// Unbalanced condor.
        UnbalancedCondor = "UNBALANCED_CONDOR",
        /// Unbalanced iron condor.
        UnbalancedIronCondor = "UNBALANCED_IRON_CONDOR",
        /// Unbalanced vertical roll.
        UnbalancedVerticalRoll = "UNBALANCED_VERTICAL_ROLL",
        /// Mutual-fund swap.
        MutualFundSwap = "MUTUAL_FUND_SWAP",
        /// Custom strategy that does not match a named pattern.
        Custom = "CUSTOM",
    }
}

string_enum! {
    /// Trade direction / intent for an order leg.
    Instruction {
        /// Buy.
        Buy = "BUY",
        /// Sell.
        Sell = "SELL",
        /// Buy shares to cover an existing short position.
        BuyToCover = "BUY_TO_COVER",
        /// Open a new short position.
        SellShort = "SELL_SHORT",
        /// Open a long option position.
        BuyToOpen = "BUY_TO_OPEN",
        /// Close a long option position.
        BuyToClose = "BUY_TO_CLOSE",
        /// Open a short option position.
        SellToOpen = "SELL_TO_OPEN",
        /// Close a short option position.
        SellToClose = "SELL_TO_CLOSE",
        /// Mutual-fund exchange.
        Exchange = "EXCHANGE",
        /// Short sale exempt from the SEC short-sale price test.
        SellShortExempt = "SELL_SHORT_EXEMPT",
    }
}

string_enum! {
    /// Lifecycle status of an order in Schwab's system.
    ApiOrderStatus {
        /// Waiting for a parent order in a multi-leg strategy.
        AwaitingParentOrder = "AWAITING_PARENT_ORDER",
        /// Waiting for a trigger condition to be met.
        AwaitingCondition = "AWAITING_CONDITION",
        /// Waiting for a stop condition to be met.
        AwaitingStopCondition = "AWAITING_STOP_CONDITION",
        /// Pending manual review by a Schwab rep.
        AwaitingManualReview = "AWAITING_MANUAL_REVIEW",
        /// Schwab accepted the order.
        Accepted = "ACCEPTED",
        /// Awaiting an "unable to route" outcome.
        AwaitingUrOut = "AWAITING_UR_OUT",
        /// Activation pending (e.g. for stop / trigger orders).
        PendingActivation = "PENDING_ACTIVATION",
        /// Queued for routing to a venue.
        Queued = "QUEUED",
        /// Live at the venue and working for a fill.
        Working = "WORKING",
        /// Schwab or the venue rejected the order.
        Rejected = "REJECTED",
        /// Cancel request submitted, not yet confirmed.
        PendingCancel = "PENDING_CANCEL",
        /// Cancelled.
        Canceled = "CANCELED",
        /// Replace request submitted, not yet confirmed.
        PendingReplace = "PENDING_REPLACE",
        /// Replaced; the original order is no longer valid.
        Replaced = "REPLACED",
        /// Filled in full.
        Filled = "FILLED",
        /// Expired (e.g. unfilled day order at session end).
        Expired = "EXPIRED",
        /// Newly submitted, not yet acked.
        New = "NEW",
        /// Waiting for a scheduled release time.
        AwaitingReleaseTime = "AWAITING_RELEASE_TIME",
        /// Awaiting venue acknowledgement.
        PendingAcknowledgement = "PENDING_ACKNOWLEDGEMENT",
        /// Recall request submitted.
        PendingRecall = "PENDING_RECALL",
        /// Schwab sent the literal string `"UNKNOWN"`.
        UnknownSchwab = "UNKNOWN",
    }
}

string_enum! {
    /// What price feed triggers a stop order.
    StopType {
        /// Default stop type for the venue.
        Standard = "STANDARD",
        /// Trigger when the bid touches the stop.
        Bid = "BID",
        /// Trigger when the ask touches the stop.
        Ask = "ASK",
        /// Trigger when the last trade touches the stop.
        Last = "LAST",
        /// Trigger when the mark touches the stop.
        Mark = "MARK",
    }
}

string_enum! {
    /// Reference price for a linked stop.
    StopPriceLinkBasis {
        /// Caller supplies the stop price directly.
        Manual = "MANUAL",
        /// Tied to the order's base price.
        Base = "BASE",
        /// Tied to the order's trigger price.
        Trigger = "TRIGGER",
        /// Tied to the instrument's last trade.
        Last = "LAST",
        /// Tied to the bid.
        Bid = "BID",
        /// Tied to the ask.
        Ask = "ASK",
        /// Tied to the ask / bid spread.
        AskBid = "ASK_BID",
        /// Tied to the mark.
        Mark = "MARK",
        /// Tied to an averaged reference price.
        Average = "AVERAGE",
    }
}

string_enum! {
    /// How the stop offset is interpreted.
    StopPriceLinkType {
        /// Absolute dollar offset.
        Value = "VALUE",
        /// Percentage offset.
        Percent = "PERCENT",
        /// Offset measured in ticks.
        Tick = "TICK",
    }
}

string_enum! {
    /// Reference price for a linked limit.
    PriceLinkBasis {
        /// Caller supplies the limit price directly.
        Manual = "MANUAL",
        /// Tied to the order's base price.
        Base = "BASE",
        /// Tied to the order's trigger price.
        Trigger = "TRIGGER",
        /// Tied to the instrument's last trade.
        Last = "LAST",
        /// Tied to the bid.
        Bid = "BID",
        /// Tied to the ask.
        Ask = "ASK",
        /// Tied to the ask / bid spread.
        AskBid = "ASK_BID",
        /// Tied to the mark.
        Mark = "MARK",
        /// Tied to an averaged reference price.
        Average = "AVERAGE",
    }
}

string_enum! {
    /// How the limit offset is interpreted.
    PriceLinkType {
        /// Absolute dollar offset.
        Value = "VALUE",
        /// Percentage offset.
        Percent = "PERCENT",
        /// Offset measured in ticks.
        Tick = "TICK",
    }
}

string_enum! {
    /// Tax-lot relief method to apply when closing positions.
    TaxLotMethod {
        /// First-in, first-out.
        Fifo = "FIFO",
        /// Last-in, first-out.
        Lifo = "LIFO",
        /// Highest cost basis first.
        HighCost = "HIGH_COST",
        /// Lowest cost basis first.
        LowCost = "LOW_COST",
        /// Average cost basis.
        AverageCost = "AVERAGE_COST",
        /// Caller-specified lot.
        SpecificLot = "SPECIFIC_LOT",
        /// Schwab's loss-harvester selection.
        LossHarvester = "LOSS_HARVESTER",
    }
}

string_enum! {
    /// Special-instruction flag attached to an order.
    SpecialInstruction {
        /// All-or-none: do not fill partially.
        AllOrNone = "ALL_OR_NONE",
        /// Do-not-reduce on ex-dividend day.
        DoNotReduce = "DO_NOT_REDUCE",
        /// Both all-or-none and do-not-reduce.
        AllOrNoneDoNotReduce = "ALL_OR_NONE_DO_NOT_REDUCE",
    }
}

string_enum! {
    /// Explicit venue the order should be routed to.
    RequestedDestination {
        /// INET (Nasdaq's ECN).
        Inet = "INET",
        /// NYSE Arca ECN.
        EcnArca = "ECN_ARCA",
        /// Cboe.
        Cboe = "CBOE",
        /// NYSE American (formerly AMEX).
        Amex = "AMEX",
        /// Philadelphia Stock Exchange.
        Phlx = "PHLX",
        /// International Securities Exchange.
        Ise = "ISE",
        /// Boston Options Exchange.
        Box_ = "BOX",
        /// New York Stock Exchange.
        Nyse = "NYSE",
        /// Nasdaq.
        Nasdaq = "NASDAQ",
        /// BATS Global Markets.
        Bats = "BATS",
        /// Cboe C2.
        C2 = "C2",
        /// Let Schwab choose the venue.
        Auto = "AUTO",
    }
}

string_enum! {
    /// Asset class of an order leg.
    OrderLegType {
        /// Listed equity.
        Equity = "EQUITY",
        /// Listed option.
        Option = "OPTION",
        /// Index.
        Index = "INDEX",
        /// Mutual fund.
        MutualFund = "MUTUAL_FUND",
        /// Cash equivalent.
        CashEquivalent = "CASH_EQUIVALENT",
        /// Fixed income.
        FixedIncome = "FIXED_INCOME",
        /// Currency.
        Currency = "CURRENCY",
        /// Collective investment vehicle.
        CollectiveInvestment = "COLLECTIVE_INVESTMENT",
    }
}

string_enum! {
    /// Whether a leg opens or closes a position.
    PositionEffect {
        /// Opening a new position.
        Opening = "OPENING",
        /// Closing an existing position.
        Closing = "CLOSING",
        /// Schwab determines the effect automatically.
        Automatic = "AUTOMATIC",
    }
}

string_enum! {
    /// How a leg's quantity is denominated.
    QuantityType {
        /// Close out the entire existing position.
        AllShares = "ALL_SHARES",
        /// Dollar-denominated (fractional shares).
        Dollars = "DOLLARS",
        /// Whole-share count.
        Shares = "SHARES",
    }
}

string_enum! {
    /// Mutual-fund dividend / capital-gains handling.
    DivCapGains {
        /// Reinvest distributions back into the fund.
        Reinvest = "REINVEST",
        /// Pay out distributions as cash.
        Payout = "PAYOUT",
    }
}

string_enum! {
    /// Lifecycle event kind on an order's activity history.
    OrderActivityType {
        /// Execution (fill).
        Execution = "EXECUTION",
        /// Order lifecycle action (place / replace / cancel).
        OrderAction = "ORDER_ACTION",
    }
}

string_enum! {
    /// Execution-event kind. Schwab currently only emits `FILL`.
    ExecutionType {
        /// Fill (partial or complete).
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
