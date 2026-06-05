//! Schwab-defined enums shared across more than one endpoint family.
//!
//! These string-valued enums describe values Schwab sends on both the REST
//! API (quotes, option chains, instruments) and the streaming API.

use crate::macros::string_enum;

string_enum! {
    /// Asset class discriminator (the `assetMainType` field).
    AssetMainType {
        /// Bond. Schwab returns no typed schema for bond quotes.
        Bond = "BOND",
        /// Equity.
        Equity = "EQUITY",
        /// Forex pair.
        Forex = "FOREX",
        /// Futures contract.
        Future = "FUTURE",
        /// Futures option.
        FutureOption = "FUTURE_OPTION",
        /// Index.
        Index = "INDEX",
        /// Mutual fund.
        MutualFund = "MUTUAL_FUND",
        /// Listed option.
        Option = "OPTION",
    }
}

string_enum! {
    /// Asset sub-type (the `assetSubType` field; only applicable to some
    /// asset classes). Mutual-fund sub-types use
    /// [`MutualFundAssetSubType`](crate::market_data::MutualFundAssetSubType).
    AssetSubType {
        /// Common stock.
        Coe = "COE",
        /// Preferred stock.
        Prf = "PRF",
        /// American Depositary Receipt.
        Adr = "ADR",
        /// Global Depositary Receipt.
        Gdr = "GDR",
        /// Closed-end fund.
        Cef = "CEF",
        /// Exchange-traded fund.
        Etf = "ETF",
        /// Exchange-traded note.
        Etn = "ETN",
        /// Unit investment trust.
        Uit = "UIT",
        /// Warrant.
        War = "WAR",
        /// Right.
        Rgt = "RGT",
    }
}

string_enum! {
    /// Call/put discriminator on an option or future-option reference.
    OptionContractType {
        /// Put.
        Put = "P",
        /// Call.
        Call = "C",
    }
}

string_enum! {
    /// Option exercise style.
    ExerciseType {
        /// American-style: exercisable any time before expiration.
        American = "A",
        /// European-style: exercisable only at expiration.
        European = "E",
    }
}

string_enum! {
    /// Option contract settlement time.
    SettlementType {
        /// AM settlement.
        Am = "A",
        /// PM settlement.
        Pm = "P",
    }
}

string_enum! {
    /// Fund-strategy code: A=Active, L=Leveraged, P=Passive,
    /// Q=Quantitative, S=Short.
    FundStrategy {
        /// Actively managed.
        Active = "A",
        /// Leveraged.
        Leveraged = "L",
        /// Passive/index-tracking.
        Passive = "P",
        /// Quantitative/rules-based.
        Quantitative = "Q",
        /// Inverse/short.
        Short = "S",
    }
}

string_enum! {
    /// A symbol's current trading status.
    SecurityStatus {
        /// Trading normally.
        Normal = "Normal",
        /// Trading halted.
        Halted = "Halted",
        /// Closed for trading.
        Closed = "Closed",
        /// Schwab's explicit `"Unknown"` status (e.g. instruments with no
        /// meaningful intraday status, such as indices or mutual funds).
        Indeterminate = "Unknown",
    }
}
