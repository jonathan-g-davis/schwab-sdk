//! Schwab streamer WebSocket.
//!
//! The streamer is a single multiplexed WebSocket: one connection carries
//! every subscribed service (level-one quotes, chart bars, book ladders,
//! screeners, account activity, admin). [`SchwabClient::streamer`](crate::SchwabClient::streamer)
//! opens it and returns a read/write pair:
//!
//! - [`WriteHalf`] sends commands. Call [`WriteHalf::login`] first; every
//!   subscribe/add/unsubscribe/view request goes through this side.
//! - [`ReadHalf`] receives frames. Each [`ReadHalf::recv`] call yields one
//!   [`StreamerResponse`] (data, response, heartbeat). Frame parsing
//!   happens inline; no background task is spawned.
//!
//! Both halves share the underlying socket through an internal mutex, so
//! they may be moved into separate tasks freely.
//!
//! Subscribe entry points on [`WriteHalf`] (e.g. [`WriteHalf::equities`]
//! for LEVELONE_EQUITIES, [`WriteHalf::chart_equity`] for CHART_EQUITY)
//! return a typed [`SubscribeRequest`] that takes keys, fields, and the
//! [`SubscriptionCommand`] (subscribe/add/unsubscribe/view).
//!
//! Connection lifecycle is exposed via [`ReadHalf::events`], a
//! `tokio::sync::watch` channel of [`ConnectionEvent`].
//!
//! # Examples
//!
//! Connect, log in, subscribe to level-one equities, and read ticks. The
//! write half is cheap to clone and drives commands; the read half is
//! polled with [`ReadHalf::recv`].
//!
//! ```no_run
//! use schwab_sdk::{AuthToken, SchwabClient, StreamerResponse};
//! use schwab_sdk::streamer::DataContent;
//! use schwab_sdk::streamer::level_one::equities::Field;
//!
//! # async fn run() -> schwab_sdk::Result<()> {
//! let client = SchwabClient::new(AuthToken::new("token"));
//!
//! let (mut read, write) = client.streamer().await?;
//! // The bearer is pulled from the client's token provider.
//! write.login().await?;
//!
//! write
//!     .equities()
//!     .subscribe(["AAPL", "MSFT"])
//!     .fields([Field::Symbol, Field::BidPrice, Field::AskPrice, Field::LastPrice])
//!     .send()
//!     .await?;
//!
//! loop {
//!     match read.recv().await? {
//!         StreamerResponse::Data(payloads) => {
//!             for payload in payloads {
//!                 if let DataContent::LevelOneEquities(ticks) = payload.content {
//!                     for tick in ticks {
//!                         println!("{}: {:?}", tick.key, tick.last_price);
//!                     }
//!                 }
//!             }
//!         }
//!         StreamerResponse::Notify(_) => { /* heartbeat */ }
//!         StreamerResponse::Response(acks) => {
//!             for ack in acks {
//!                 println!("{:?} {:?}: {}", ack.service, ack.command, ack.content.message);
//!             }
//!         }
//!         // `StreamerResponse` is non-exhaustive.
//!         _ => {}
//!     }
//! }
//! # }
//! ```

mod admin;
mod connection;
mod events;
mod protocol;
mod request;
mod response;

pub mod account_activity;
pub mod book;
pub mod chart;
pub mod level_one;
pub mod screener;
pub mod subscription;

pub use connection::{ReadHalf, WebSocketError, WriteHalf, connect};
pub use events::{ConnectionEvent, DisconnectReason};
pub use protocol::{ResponseCode, Service, StreamerCommand};
pub(crate) use request::StreamerRequest;
pub use response::{
    DataContent, DataPayload, Heartbeat, ResponseContent, ResponsePayload, StreamerResponse,
};
pub use subscription::{Command as SubscriptionCommand, SubscribeRequest};
