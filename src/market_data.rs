//! Schwab Market Data API.
//!
//! Reached through [`SchwabClient::market_data`](crate::SchwabClient::market_data).
//! All endpoints in this family hit a different base URL than the Trader
//! API ([`crate::MARKET_DATA_BASE_URL`] vs [`crate::TRADER_BASE_URL`]).
//!
//! # Examples
//!
//! Snapshot quotes for several symbols at once. An invalid symbol does not
//! fail the batch; it comes back as a [`QuoteEntry::Error`] entry in the
//! response map.
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient};
//! use schwab_sdk::market_data::QuoteEntry;
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! let quotes = client.market_data().quotes().list(["AAPL", "MSFT", "SPY"]).send().await?;
//! for (symbol, entry) in &quotes {
//!     match entry {
//!         QuoteEntry::Equity(q) => {
//!             let last = q.quote.as_ref().and_then(|inner| inner.last_price);
//!             println!("{symbol}: {last:?}");
//!         }
//!         QuoteEntry::Error(_) => println!("{symbol}: not found"),
//!         _ => println!("{symbol}: non-equity asset"),
//!     }
//! }
//! # Ok(())
//! # }
//! ```

mod chains;
mod expiration_chain;
mod instruments;
mod market_hours;
mod movers;
mod price_history;
mod quotes;

pub use chains::{
    Chains, ContractType, Entitlement, ExpirationMonth, ExpirationType, GetChainBuilder,
    OptionChain, OptionContract, OptionContractMap, OptionDeliverables, OptionRange,
    OptionStrategy, OptionType, PutCall, SettlementType, Underlying, UnderlyingExchange,
};
pub use expiration_chain::{Expiration, ExpirationChain, ExpirationChainResponse};
pub use instruments::{
    Bond, FundamentalInst, Instrument, InstrumentAssetType, InstrumentResponse, Instruments,
    InstrumentsResponse, Projection,
};
pub use market_hours::{
    GetMarketHoursBuilder, Hours, Interval, ListMarketHoursBuilder, Market, MarketHours,
    MarketHoursResponse, MarketType,
};
pub use movers::{
    GetMoversBuilder, MoverDirection, MoverIndex, MoverSort, Movers, MoversResponse, Screener,
};
pub use price_history::{
    Candle, CandleList, FrequencyType, GetPriceHistoryBuilder, PeriodType, PriceHistory,
};
pub use quotes::{
    AssetMainType, AssetSubType, EquityQuote, ExerciseType, ExtendedMarket, ForexQuote,
    FundStrategy, Fundamental, FutureOptionQuote, FutureQuote, GetQuoteBuilder, IndexQuote,
    ListQuotesBuilder, MutualFundAssetSubType, MutualFundQuote, OptionContractType, OptionQuote,
    QuoteEntry, QuoteEquity, QuoteError, QuoteField, QuoteForex, QuoteFuture, QuoteFutureOption,
    QuoteIndex, QuoteMutualFund, QuoteOption, QuoteResponse, QuoteType, Quotes, ReferenceEquity,
    ReferenceForex, ReferenceFuture, ReferenceFutureOption, ReferenceIndex, ReferenceMutualFund,
    ReferenceOption, RegularMarket,
};

use crate::client::SchwabClient;

/// Accessor for the Market Data API endpoint families. Construct via
/// [`SchwabClient::market_data`](crate::SchwabClient::market_data).
#[derive(Debug)]
pub struct MarketData<'a> {
    client: &'a SchwabClient,
}

impl<'a> MarketData<'a> {
    pub(crate) fn new(client: &'a SchwabClient) -> Self {
        Self { client }
    }

    /// Accessor for `/quotes` and `/{symbol}/quotes` - snapshot quotes
    /// for one or more symbols across every supported asset class.
    pub fn quotes(&self) -> Quotes<'a> {
        Quotes::new(self.client)
    }

    /// Accessor for `/pricehistory` - OHLCV candles for a single symbol
    /// at a configurable aggregation.
    pub fn price_history(&self) -> PriceHistory<'a> {
        PriceHistory::new(self.client)
    }

    /// Accessor for `/markets*` - market hours and session windows for
    /// one or more markets on a given date.
    pub fn market_hours(&self) -> MarketHours<'a> {
        MarketHours::new(self.client)
    }

    /// Accessor for `/movers/{symbol_id}` - top-moving securities
    /// within an index.
    pub fn movers(&self) -> Movers<'a> {
        Movers::new(self.client)
    }

    /// Accessor for `/instruments*` - instrument search by symbol /
    /// description and lookup by CUSIP.
    pub fn instruments(&self) -> Instruments<'a> {
        Instruments::new(self.client)
    }

    /// Accessor for `/chains` - the option chain for an optionable
    /// symbol, grouped by expiration and strike.
    pub fn chains(&self) -> Chains<'a> {
        Chains::new(self.client)
    }

    /// Accessor for `/expirationchain` - the option expiration series
    /// for an optionable symbol, without per-expiration contracts.
    pub fn expiration_chain(&self) -> ExpirationChain<'a> {
        ExpirationChain::new(self.client)
    }
}
