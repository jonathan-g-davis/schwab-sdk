//! Schwab "Level 1" streamer services.
//!
//! Each submodule covers one service:
//!
//! - [`equities`] - `LEVELONE_EQUITIES`
//! - [`options`] - `LEVELONE_OPTIONS`
//! - [`futures`] - `LEVELONE_FUTURES`
//! - [`futures_options`] - `LEVELONE_FUTURES_OPTIONS`
//! - [`forex`] - `LEVELONE_FOREX`
//!
//! Every service provides a `Field` enum (one variant per documented field)
//! and a typed `Content` struct that consumers receive through
//! [`DataContent`].
//!
//! Delivery type for all LEVELONE_* services is "Change": only the fields
//! that changed since the previous tick are present, so all numerically-
//! indexed fields are `Option<T>`.

pub mod equities;
pub mod futures;
pub mod options;
