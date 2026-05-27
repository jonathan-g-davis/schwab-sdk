# schwab-sdk

[![crates.io](https://img.shields.io/crates/v/schwab-sdk.svg)](https://crates.io/crates/schwab-sdk)
[![Documentation](https://docs.rs/schwab-sdk/badge.svg)](https://docs.rs/schwab-sdk)
[![CI](https://github.com/jonathan-g-davis/schwab-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/jonathan-g-davis/schwab-sdk/actions/workflows/ci.yml)
[![MIT/Apache 2.0 licensed](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A typed Rust client for the [Charles Schwab Trader API][trader], Market Data,
and streaming data.

It provides access to every endpoint via a namespace accessor on
[`SchwabClient`]. With it, you can:

- [List linked accounts, balances, and their positions][`accounts`]
- [Query quotes, price history, options chains, and other market data][`market_data`]
- [Stream real-time market data and account activity][`streamer`]
- [Place, replace, cancel, and preview orders][`orders::Orders`]
- [List transactions][`transactions`]
- [Read user preferences][`user_preferences`]

Money and quantity fields use [`rust_decimal::Decimal`]. Bearer tokens and
account identifiers are wrapped in [`secrecy`] newtypes that redact in `Debug`
and zeroise on drop.

API documentation: [docs.rs/schwab-sdk][docs].

## Quickstart

Resolve an account, read a quote, and place a limit buy under the last
trade:

```rust
use rust_decimal_macros::dec;
use schwab_sdk::{AuthToken, SchwabClient};
use schwab_sdk::market_data::QuoteEntry;
use schwab_sdk::orders::OrderRequest;

#[tokio::main]
async fn main() -> schwab_sdk::Result<()> {
    let client = SchwabClient::new(AuthToken::new("your access token"));

    // Per-account endpoints take the encrypted account hash, not the
    // plain account number. Resolve it once from /accounts/accountNumbers.
    let accounts = client.accounts().numbers().await?;
    let account_hash = &accounts.first().expect("a linked account").hash_value;

    // Read a quote. Unknown symbols come back as QuoteEntry::Error, not Err.
    let quotes = client.market_data().quotes().list(["AAPL"]).send().await?;
    let Some(QuoteEntry::Equity(q)) = quotes.get("AAPL") else { return Ok(()) };
    let Some(last_price) = q.quote.as_ref().and_then(|inner| inner.last_price) else {
        return Ok(());
    };

    // Place a limit buy just below the quote and print the order id Schwab returns.
    let order_id = client
        .orders(account_hash)
        .place(OrderRequest::buy_limit("AAPL", dec!(10), last_price - dec!(0.50)))
        .await?;
    println!("placed order {order_id}");

    Ok(())
}
```

## License

Licensed under either of

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

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
[`SchwabClient`]: https://docs.rs/schwab-sdk/latest/schwab_sdk/struct.SchwabClient.html
[`accounts`]: https://docs.rs/schwab-sdk/latest/schwab_sdk/accounts/index.html
[`market_data`]: https://docs.rs/schwab-sdk/latest/schwab_sdk/market_data/index.html
[`streamer`]: https://docs.rs/schwab-sdk/latest/schwab_sdk/streamer/index.html
[`orders::Orders`]: https://docs.rs/schwab-sdk/latest/schwab_sdk/orders/struct.Orders.html
[`transactions`]: https://docs.rs/schwab-sdk/latest/schwab_sdk/transactions/index.html
[`user_preferences`]: https://docs.rs/schwab-sdk/latest/schwab_sdk/user_preferences/index.html
