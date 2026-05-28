# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/jonathan-g-davis/schwab-sdk/compare/v0.1.0...v0.1.1) - 2026-05-28

### Other

- Setup continuous deployment ([#9](https://github.com/jonathan-g-davis/schwab-sdk/pull/9))
- Docs cleanup ([#8](https://github.com/jonathan-g-davis/schwab-sdk/pull/8))
- Update redactions ([#7](https://github.com/jonathan-g-davis/schwab-sdk/pull/7))
- Fix mis-documented types ([#6](https://github.com/jonathan-g-davis/schwab-sdk/pull/6))

## [0.1.0] - 2026-05-26

Initial release. A typed Rust client for the Charles Schwab Trader API, Market
Data APIs, and streaming data, exposed through namespace accessors on
`SchwabClient`.

### Added

- **Accounts** (`client.accounts()`): list linked account numbers and their
  encrypted hashes, read balances, and read positions.
- **Market data** (`client.market_data()`): quotes, price history, option
  chains, option expiration chains, movers, market hours, and instrument
  search. Unknown symbols surface as a `QuoteEntry::Error` variant rather than
  a request error.
- **Orders** (`client.orders(account_hash)`): place, replace, cancel, and
  preview orders via the compile-time-checked `OrderRequest` builder.
- **Transactions** (`client.transactions()`): list account transactions.
- **User preferences** (`client.user_preferences()`): read user preferences.
- **Streamer** (`client.streamer()`): real-time market-data and account-activity
  streaming over WSS, with reconnect support.
- **Authentication**: `TokenProvider` trait consulted once per REST request and
  once per streamer LOGIN frame, so a rotated token is observed on the next call
  without rebuilding the client; `StaticTokenProvider` for fixed tokens.
- Re-exports of `chrono`, `http`, and `rust_decimal` for types that appear in
  the public API.

### Security

- Secret newtypes (`AuthToken`, `CustomerId`, `AccountNumber`, `AccountHash`)
  redact in `Debug` and zeroize on `Drop`.
- The crate emits no log lines, writes no files, and embeds no secret values in
  `Error` variants. A bearer credential is materialized only at the
  `Authorization` header and the streamer LOGIN frame.
- Transport defaults to HTTPS for REST and WSS for the streamer. Release builds
  reject `http://` base-URL overrides and `ws://` streamer URLs; debug builds
  permit them so local fixture servers work in tests.

### Notes

- All money and quantity fields use `rust_decimal::Decimal`.
