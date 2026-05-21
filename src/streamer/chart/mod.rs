//! Chart (candle) streamer services.
//!
//! Each service publishes minute-resolution OHLCV candles. Delivery type is
//! "All Sequence": every tick is sent (not conflated by the streamer) and
//! carries a sequence number identifying its candle.
//!
//! `equity` and `futures` use different field orderings on the wire, so they
//! ship as separate modules with independent `Field` enums and `Content`
//! structs.

pub mod equity;
