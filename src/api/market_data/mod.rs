//! Schwab Market Data API.
//!
//! Reached through [`SchwabClient::market_data`](crate::SchwabClient::market_data).
//! All endpoints in this family hit a different base URL than the Trader
//! API ([`crate::rest::MARKET_DATA_BASE_URL`] vs
//! [`crate::rest::TRADER_BASE_URL`]).

pub mod market_hours;
pub mod movers;
pub mod price_history;
pub mod quotes;

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
    AssetMainType, AssetSubType, EquityQuote, ExtendedMarket, FundStrategy, Fundamental,
    GetQuoteBuilder, ListQuotesBuilder, QuoteEntry, QuoteEquity, QuoteError, QuoteField,
    QuoteResponse, QuoteType, Quotes, ReferenceEquity, RegularMarket,
};

use crate::rest::SchwabClient;

/// Accessor for the Market Data API endpoint families. Construct via
/// [`SchwabClient::market_data`](crate::SchwabClient::market_data).
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
}
