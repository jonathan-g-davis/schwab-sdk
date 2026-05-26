# schwab-sdk

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

`schwab-sdk` is a faithful, typed mapping of Schwab's API. It provides
building blocks, not a framework. The crate is intended to empower developers
to create their own application quickly and safely without getting in the way.

- **Building blocks without policy.** The crate types every REST endpoint and
  streamer service and stops there. Credential storage, the OAuth
  authorization flow, retry loops, rate limiters, reconnect-and-resubscribe,
  and any strategy / portfolio / risk logic are deliberately left to the
  caller. Where a consumer needs to plug in behavior, the crate exposes a
  seam (see below) rather than shipping a default that takes over.

- **No panics.** The crate does not contain `panic!()`, `expect()`, or `unwrap()`
  in any production code. Where operations are fallible (even if unlikely), a
  `Result` is returned instead.

- **Spec implemented as written; notable exceptions called out explicitly.**
  Field names, request shapes, and the documented enum values follow Schwab's
  published schema. Where the wire is ambiguous or still evolving, decoding
  stays permissive: every enum carries an `Unknown` / `Raw` fallback, so a
  discriminant or service Schwab adds later deserializes into a catch-all variant
  (with the raw value preserved) instead of failing the whole response. An invalid
  symbol in a quote batch comes back as a typed error entry, not an `Err`.

- **Types that make mistakes hard.** Every price, quantity, and money field
  is [`rust_decimal::Decimal`], never `f64`, so no precision is lost at the
  boundary. Bearer tokens, customer ids, and account numbers are
  [`secrecy`]-backed newtypes that redact in `Debug` and zeroize on drop, so
  a credential cannot leak through a stray `dbg!` or log line. The streamer's
  subscribe builders are typestate: picking a verb and a field set is checked
  at compile time.

- **Errors are structured, retry is a seam.** A single `thiserror` enum
  surfaces every failure; Schwab's two distinct error-body shapes (Trader vs
  Market Data) are both preserved. The crate never retries for you -
  `Error::is_retryable` and `Error::retry_after` classify a failure so you
  can layer `backon` or any policy on top. Token rotation goes through the
  `TokenProvider` trait, and streamer connection state is published on a
  `ConnectionEvent` watch channel so a reconnect loop lives in your code.

- **Forward compatible.** Public enums and response structs are
  `#[non_exhaustive]`, so Schwab adding a field or variant is a non-breaking
  change for downstreams rather than a new major version.

- **Tested at the wire boundary.** Every request and response type has
  serialization round-trip coverage, and the streamer frame parser is tested
  against captured frames. The suite runs against mocked transports. No live
  Schwab session or credentials are required for `cargo test`.

- **Runtime.** Built on Tokio. No async runtime is started for you, and once
  the streamer is connected, no background task drives it. Reads and writes
  happen inline on the task that calls `recv` / `send`, so you decide how the
  halves are scheduled.

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

For a short-lived token where the application tears down the client when
it expires, use `SchwabClient::new`. It wraps the supplied `AuthToken`
in a `StaticTokenProvider` internally:

```rust
use schwab_sdk::{AuthToken, SchwabClient};

let client = SchwabClient::new(AuthToken::new(env!("SCHWAB_AUTH_TOKEN")));
```

For long-lived clients, implement `TokenProvider` over whatever cell or
refresh strategy fits your application. A swappable provider using
`arc-swap` for wait-free reads. Your refresh loop calls `rotate` when a
new access token arrives, and the next REST call (or streamer LOGIN)
hands it out:

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
client-supplied idempotency key, so placing an order is *not* safe to retry.
If a `place` call fails after the request reached Schwab (a timeout, a
dropped connection, a 5xx), the order may have been accepted even though you
received an `Err`. There is no key you can resend to deduplicate it.

The recovery pattern is to reconcile before determing whether to resubmit:

1. Record the time just before calling `place`.
2. If `place` returns a retryable error, list the orders entered since that
   time with `client.orders(account_hash).list(from, to)`.
3. Match the returned orders by symbol, side, and quantity. If one matches,
   the order landed - adopt its id. If none does, it is safe to resubmit.

The same applies to `replace`, which Schwab implements as a cancel-and-place.

## Security

`schwab-sdk` is built to reduce the risk of credential or PII leakage
through this crate. It is not a security boundary for your application
as a whole, and it makes no warranty; the MIT and Apache-2.0 licences
under which it is distributed disclaim that explicitly. See
[`SECURITY.md`](SECURITY.md) for the vulnerability-reporting channel
and the formal scope.

### What the SDK does

- **Secret newtypes.** [`AuthToken`][AuthToken-doc],
  [`CustomerId`][CustomerId-doc], [`AccountNumber`][AccountNumber-doc],
  and [`AccountHash`][AccountHash-doc] are `secrecy::SecretBox`-backed
  newtypes. `Debug` prints `Secret([REDACTED ...])`. `Drop` zeroises
  the inner buffer. `Clone` copies the protected box rather than
  producing a plain `String`. The raw value is reachable only via
  `.expose_secret()`. The [`secrets`][secrets-doc] module documents
  what these properties cover and what they do not (debuggers, core
  dumps, swap, panic hooks, and any caller-side code that copies the
  exposed value into a `String`).

- **No logs or file writes, no secrets in errors.** The crate has no
  `println!`, `eprintln!`, `tracing`, or `log` calls in production
  code paths. It never writes to disk. No `Error` or `WebSocketError`
  variant carries a bearer or a `SecretBox`-wrapped value. A bearer is
  materialised in exactly two places on the wire: the
  `Authorization: Bearer ...` header on outbound REST requests and the
  streamer LOGIN frame's `Authorization` field.

- **HTTPS for REST, WSS for the streamer.** Default base URLs point at
  Schwab's production HTTPS endpoints. The `with_trader_base_url` and
  `with_market_data_base_url` builders accept caller-supplied URLs but
  reject any scheme other than `https://` in release builds. The
  streamer connect path rejects any scheme other than `wss://` in
  release builds, even though the URL is supplied by Schwab's own
  `/userPreference` response. Debug builds additionally permit `http://`
  and `ws://` so wiremock and other local fixture servers work in
  tests. **Production deployments must use release builds**; a debug
  binary in production would weaken these checks.

- **Forward-compatible decoding avoids panic-on-input.** Public enums
  and response structs are `#[non_exhaustive]`; unknown discriminants
  land in `Unknown` / `Raw` fallback variants with the raw value
  preserved. A Schwab response containing a new field or service does
  not panic the SDK and does not abort the connection. The panic-family
  lints (`unwrap`, `expect`, `panic`, `unreachable`, `todo`,
  `unimplemented`) are denied in non-test code at compile time.

- **Manual `Debug` where automatic would widen.** Types that hold a
  `TokenProvider` derive `Debug` by hand so `dbg!(&client)` prints the
  provider as a placeholder rather than its contents.

### What you must do

- **Storage.** The SDK does not persist tokens. Put the refresh token
  in an OS-native credential store (Keychain on macOS, Credential
  Manager on Windows, kernel keyutils via `keyring` / `keyring-core`
  on Linux). Do not commit tokens to `.env`, config files, or CI
  environment variables visible across jobs. Refresh tokens are bearer
  credentials with trading authority on a real-money account; treat
  them at that sensitivity.

- **Process exposure.** A token in a process's environment is readable
  by any process running as the same user, and by `/proc/<pid>/environ`
  on Linux. Prefer reading from a credential store at startup over
  `std::env::var` in production binaries. Never use `env!` for a real
  token as it bakes the value into the binary at compile time.

- **`expose_secret()` is the security boundary.** Each call site
  should be one of: bearer header construction, LOGIN-frame
  construction, or credential-store encode. New call sites should fail
  code review by default. `expose_secret().to_string()` defeats the
  newtype.

- **OAuth flow.** The SDK does not perform the authorization-code
  exchange. If you stand up a local callback server: bind to
  `127.0.0.1` only, make the listener one-shot, and validate the
  `state` parameter on every callback to prevent CSRF.

- **Logging discipline.** If you wrap calls in `tracing` or similar,
  redact request bodies and headers. The streamer LOGIN frame
  serialises the auth token into JSON before transmission, so logging
  a constructed frame body would leak a bearer even though
  [`AuthToken`][AuthToken-doc] itself redacts in its own `Debug`.
  Either keep frame-level logging off, or scrub by field.

- **Account number vs. account hash.** Every per-account REST endpoint
  takes the encrypted [`AccountHash`][AccountHash-doc], never the
  plain [`AccountNumber`][AccountNumber-doc]. If you carry the plain
  number through your own logs, metrics, or error reports, it is PII;
  treat it accordingly.

- **Data at rest.** `secrecy` zeroises on `Drop`. It does not
  protect against a debugger attached to the live process, a core dump
  that captures heap pages, or pages swapped to disk. These concerns
  are out of scope of this crate and must be addressed elsewhere.

- **Order resubmission.**
  A blind retry on a network failure can place a duplicate order.
  See [Retries and idempotency](#retries-and-idempotency) for the
  reconcile-before-resubmit pattern.

## License

Licensed under either of

- Apache License, Version 2.0
- MIT license

at your option.

[trader]: https://developer.schwab.com/
[docs]: https://docs.rs/schwab-sdk
[`rust_decimal::Decimal`]: https://docs.rs/rust_decimal
[`secrecy`]: https://docs.rs/secrecy
[AuthToken-doc]: https://docs.rs/schwab-sdk/latest/schwab_sdk/struct.AuthToken.html
[CustomerId-doc]: https://docs.rs/schwab-sdk/latest/schwab_sdk/struct.CustomerId.html
[AccountNumber-doc]: https://docs.rs/schwab-sdk/latest/schwab_sdk/struct.AccountNumber.html
[AccountHash-doc]: https://docs.rs/schwab-sdk/latest/schwab_sdk/struct.AccountHash.html
[secrets-doc]: https://docs.rs/schwab-sdk/latest/schwab_sdk/secrets/index.html
