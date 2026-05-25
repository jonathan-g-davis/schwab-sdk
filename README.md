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
refresh strategy fits your application. A swappable provider in ~15
lines (using `arc-swap` for wait-free reads; `RwLock<AuthToken>` works
equally well if you prefer the stdlib):

```rust
use std::sync::Arc;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use schwab_sdk::{AuthToken, Error, SchwabClient, TokenProvider};

struct SwappableProvider(ArcSwap<AuthToken>);

#[async_trait]
impl TokenProvider for SwappableProvider {
    async fn access_token(&self) -> Result<AuthToken, Error> {
        Ok((*self.0.load_full()).clone())
    }
}

let provider = Arc::new(SwappableProvider(ArcSwap::from_pointee(AuthToken::new("..."))));
let client = SchwabClient::with_token_provider(provider.clone());

// Later, after your refresh strategy obtains a new token:
provider.0.store(Arc::new(AuthToken::new("rotated")));
```

The SDK ships only `StaticTokenProvider`; refreshing providers,
persistence backends, and scheduling are application concerns.

## License

Licensed under either of

- Apache License, Version 2.0
- MIT license

at your option.

[trader]: https://developer.schwab.com/
[docs]: https://docs.rs/schwab-sdk
[`rust_decimal::Decimal`]: https://docs.rs/rust_decimal
[`secrecy`]: https://docs.rs/secrecy
