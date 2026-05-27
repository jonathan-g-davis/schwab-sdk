# schwab-sdk

[![crates.io](https://img.shields.io/crates/v/schwab-sdk.svg)](https://crates.io/crates/schwab-sdk)
[![Documentation](https://docs.rs/schwab-sdk/badge.svg)](https://docs.rs/schwab-sdk)
[![CI](https://github.com/jonathan-g-davis/schwab-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/jonathan-g-davis/schwab-sdk/actions/workflows/ci.yml)
[![MIT/Apache 2.0 licensed](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Typed Rust client for the [Charles Schwab Trader API][trader] and its
streamer WebSocket.

- REST endpoints for accounts, orders, transactions, user preferences, and the
  full market-data surface (quotes, price history, option chains, instruments,
  market hours, movers).
- A streaming client for the Schwab streamer with typed payloads for the
  level-one, book, chart, screener, and account-activity services.
- All money and quantity fields use [`rust_decimal::Decimal`]; secrets are
  wrapped in [`secrecy`] newtypes that redact in `Debug` and zeroize on drop.

What this crate does **not** do:

- It does not perform the OAuth authorization flow. Bring your own access
  token; see Schwab's developer portal for the auth-code / refresh-token
  flow.
- It does not retry failed requests. Each `Error` exposes
  [`is_retryable`](https://docs.rs/schwab-sdk/latest/schwab_sdk/enum.Error.html#method.is_retryable)
  and `retry_after` so callers can layer a policy of their choice
  (e.g. `backon`) on top.
- It does not rate-limit. Schwab does not publish per-endpoint limits at
  the time of writing; callers are responsible for staying within them.

API documentation lives at [docs.rs/schwab-sdk][docs].

## Design

- **Building blocks without policy.** Every REST endpoint and streamer service
  is typed. Credential storage, OAuth, retries, rate limits, reconnect, and any
  strategy logic are the caller's responsibility. Extension points are exposed
  as seams rather than prescribing a default. `TokenProvider` enables
  implementing your own auth policy. `ConnectionEvent` can be subscribed to for
  handling streamer reconnects as desired.

- **Spec as written and forward-compatible.** Field names, request shapes, and
  enum values follow Schwab's schema. Public enums and response structs are
  `#[non_exhaustive]` and every enum carries an `Unknown` / `Raw` fallback so a
  new Schwab discriminant or service is non-breaking.

- **Structured errors.** One `thiserror` enum surfaces every failure and
  preserves both Schwab error envelopes (Trader and Market Data).
  `Error::is_retryable` and `Error::retry_after` classify failures so callers
  can layer `backon` or another policy on top.

- **Round-trip tests.** Every request and response type has serialization
  round-trip coverage. The streamer frame parser is tested against captured
  frames. No live Schwab session is required for `cargo test`.

## Usage

Resolve an account, read a quote, and place an order against it:

```rust
use rust_decimal_macros::dec;
use schwab_sdk::{AuthToken, SchwabClient};
use schwab_sdk::market_data::QuoteEntry;
use schwab_sdk::orders::OrderRequest;

#[tokio::main]
async fn main() -> schwab_sdk::Result<()> {
    let client = SchwabClient::new(AuthToken::new("your access token"));

    // Every per-account endpoint takes the encrypted account hash, never
    // the plain account number. Resolve it once from /accounts/accountNumbers.
    let accounts = client.accounts().numbers().await?;
    let account_hash = &accounts.first().expect("a linked account").hash_value;

    // Read a quote. The response is keyed by symbol; an invalid symbol comes
    // back as QuoteEntry::Error rather than failing the whole request.
    let quotes = client.market_data().quotes().list(["AAPL"]).send().await?;
    let last_price = match quotes.get("AAPL") {
        Some(QuoteEntry::Equity(q)) => q.quote.as_ref().and_then(|inner| inner.last_price),
        _ => None,
    };
    let Some(last_price) = last_price else {
        return Ok(());
    };

    // Place a limit buy just under the last trade. Schwab returns the new
    // order id; fetch it back to watch the fill.
    let order_id = client
        .orders(account_hash)
        .place(OrderRequest::buy_limit("AAPL", dec!(10), last_price - dec!(0.50)))
        .await?;
    let order = client.orders(account_hash).get(order_id).await?;
    println!("order {order_id}: {:?}", order.status);

    Ok(())
}
```

Stream live level-one quotes. The write half sends commands (log in first);
the read half yields one typed frame per `recv`:

```rust
use schwab_sdk::{AuthToken, SchwabClient, StreamerResponse};
use schwab_sdk::streamer::DataContent;
use schwab_sdk::streamer::level_one::equities::Field;

#[tokio::main]
async fn main() -> schwab_sdk::Result<()> {
    let client = SchwabClient::new(AuthToken::new("your access token"));

    let (mut read, write) = client.streamer().await?;
    write.login().await?;

    write
        .equities()
        .subscribe(["AAPL", "MSFT"])
        .fields([Field::Symbol, Field::BidPrice, Field::AskPrice, Field::LastPrice])
        .send()
        .await?;

    loop {
        match read.recv().await? {
            StreamerResponse::Data(payloads) => {
                for payload in payloads {
                    if let DataContent::LevelOneEquities(ticks) = payload.content {
                        for tick in ticks {
                            println!("{}: {:?}", tick.key, tick.last_price);
                        }
                    }
                }
            }
            // Heartbeats and subscription acknowledgements.
            _ => {}
        }
    }
}
```

## Authentication and token rotation

`SchwabClient` reads its bearer through a `TokenProvider` trait. The SDK
consults it once per REST request and once per streamer LOGIN frame, so a
token rotated in the provider is observed on the next call without
rebuilding the client.

### Static token

For a short-lived token where the application tears down the client when
it expires, use `SchwabClient::new`. It wraps the supplied `AuthToken`
in a `StaticTokenProvider` internally:

```rust
use schwab_sdk::{AuthToken, SchwabClient};

let client = SchwabClient::new(AuthToken::new(env!("SCHWAB_AUTH_TOKEN")));
```

### Rotating token

For long-lived clients, implement `TokenProvider` over whatever cell or
refresh strategy fits your application. The example below is a swappable
provider built on `arc-swap` for wait-free reads: a refresh loop calls
`rotate` when a new access token arrives, and the next REST call (or
streamer LOGIN) hands it out.

```rust
use std::sync::Arc;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use schwab_sdk::{AuthToken, Error, SchwabClient, TokenProvider};

struct SwappableProvider(ArcSwap<AuthToken>);

impl SwappableProvider {
    fn new(initial: AuthToken) -> Self {
        Self(ArcSwap::from_pointee(initial))
    }

    /// Called by your refresh loop when a fresh access token arrives.
    fn rotate(&self, fresh: AuthToken) {
        self.0.store(Arc::new(fresh));
    }
}

#[async_trait]
impl TokenProvider for SwappableProvider {
    async fn access_token(&self) -> Result<AuthToken, Error> {
        Ok((*self.0.load_full()).clone())
    }
}

let provider = Arc::new(SwappableProvider::new(AuthToken::new("initial-token")));
let client = SchwabClient::with_token_provider(provider.clone());

// Later, after your refresh strategy obtains a new token:
provider.rotate(AuthToken::new("rotated-token"));
```

The SDK ships only `StaticTokenProvider`; refreshing providers,
persistence backends, and scheduling are application concerns.

## Retries and idempotency

`Error::is_retryable` and `Error::retry_after` classify a failure so you can
layer a backoff policy on top. Read-only and naturally idempotent requests
(quotes, account reads, order lists, cancels) can be retried directly on a
retryable error.

**Order placement is not retry-safe.** Schwab's Trader API has no
client-supplied idempotency key, so placing an order is _not_ safe to retry.
If a `place` call fails after the request reached Schwab (a timeout, a
dropped connection, a 5xx), the order may have been accepted even though you
received an `Err`. There is no key you can resend to deduplicate it.

The recovery pattern is to reconcile before deciding whether to resubmit:

1. Record the time just before calling `place`.
2. If `place` returns a retryable error, list the orders entered since that
   time with `client.orders(account_hash).list(from, to)`.
3. Match the returned orders by symbol, side, and quantity. If one matches,
   the order landed - adopt its id. If none does, it is safe to resubmit.

The same applies to `replace`, which Schwab implements as a cancel-and-place.

## Security

`schwab-sdk` is built to reduce the risk of credential or PII leakage
through this crate; it is not a security boundary for the application as
a whole. The crate ships under MIT / Apache-2.0 with no warranty.

- Bearer tokens, customer ids, and account identifiers are
  [`secrecy`]-backed newtypes that redact in `Debug` and zeroise on
  `Drop`. The raw value is reachable only via `.expose_secret()`, the
  single grep-able boundary.
- The crate has no `println!`, `tracing`, or `log` calls in production
  code; it never writes to disk; and no `Error` variant carries a
  bearer. A bearer is materialised only on the `Authorization` header
  and in the streamer LOGIN frame.
- REST and the streamer default to HTTPS / WSS. Release builds reject
  any other scheme on base-URL overrides; debug builds permit `http://`
  and `ws://` so local fixture servers can be wired up in tests.
  Production deployments must use release builds.

See the [`secrets` module docs][secrets-doc] for the full threat model
and caller-side hardening guidance (token storage, process exposure,
logging discipline, account-number vs. account-hash handling). See
[`SECURITY.md`](SECURITY.md) for the vulnerability-reporting channel
and the formal scope.

## License

Licensed under either of

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Disclaimer

This project is an independent, community-maintained client. It is **not
affiliated with, endorsed by, or sponsored by Charles Schwab & Co., Inc.**
"Schwab" and related marks are the property of their respective owners.

This software is provided "as is" without warranty of any kind. The
authors and contributors are **not responsible for any financial loss,
missed trades, incorrect or duplicate orders, or other trading outcomes**
arising from use of this crate. You are solely responsible for the orders
your code submits and for verifying its behavior before trading real
money. See the MIT and Apache-2.0 license texts for the full warranty
disclaimer.

[trader]: https://developer.schwab.com/
[docs]: https://docs.rs/schwab-sdk
[`rust_decimal::Decimal`]: https://docs.rs/rust_decimal
[`secrecy`]: https://docs.rs/secrecy
[secrets-doc]: https://docs.rs/schwab-sdk/latest/schwab_sdk/secrets/index.html
