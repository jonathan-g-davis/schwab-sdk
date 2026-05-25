//! Integration coverage for the [`TokenProvider`]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use schwab_sdk::error::Error;
use schwab_sdk::{AuthToken, SchwabClient, TokenProvider};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

/// A provider that counts calls and can be told to fail. Used to assert
/// the SDK's per-request fetch contract.
struct CountingProvider {
    token: AuthToken,
    calls: AtomicUsize,
    fail: bool,
}

impl CountingProvider {
    fn new(token: &str) -> Arc<Self> {
        Arc::new(Self {
            token: AuthToken::new(token),
            calls: AtomicUsize::new(0),
            fail: false,
        })
    }

    fn failing() -> Arc<Self> {
        Arc::new(Self {
            token: AuthToken::new("unused"),
            calls: AtomicUsize::new(0),
            fail: true,
        })
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl TokenProvider for CountingProvider {
    async fn access_token(&self) -> Result<AuthToken, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(Error::TokenProvider {
                source: "provider down".into(),
            })
        } else {
            Ok(self.token.clone())
        }
    }
}

#[tokio::test]
async fn custom_provider_is_called_once_per_request() {
    let trader = common::trader_mock().await;
    let market = common::market_mock().await;
    let provider = CountingProvider::new("rotated-token-xyz");

    Mock::given(method("GET"))
        .and(path("/accounts/accountNumbers"))
        .and(header("Authorization", "Bearer rotated-token-xyz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("accounts/numbers.json")),
        )
        .expect(3)
        .mount(&trader)
        .await;

    // wiremock yields `http://` URIs; accepted in debug builds (where
    // `cargo test` runs) and rejected otherwise.
    let client = SchwabClient::with_token_provider(provider.clone())
        .with_trader_base_url(trader.uri())
        .expect("wiremock URI is http:// which is permitted in debug builds")
        .with_market_data_base_url(market.uri())
        .expect("wiremock URI is http:// which is permitted in debug builds");

    // Three independent REST calls; the provider must be consulted on
    // each one.
    for _ in 0..3 {
        client.accounts().numbers().await.unwrap();
    }

    assert_eq!(provider.call_count(), 3);
}

#[tokio::test]
async fn provider_failure_surfaces_without_network_io() {
    let trader = common::trader_mock().await;
    let market = common::market_mock().await;
    let provider = CountingProvider::failing();

    // No mocks mounted: if the SDK reaches the network despite the
    // provider failing, wiremock returns 404 and we'd see a different
    // error variant.
    let client = SchwabClient::with_token_provider(provider.clone())
        .with_trader_base_url(trader.uri())
        .expect("wiremock URI is http:// which is permitted in debug builds")
        .with_market_data_base_url(market.uri())
        .expect("wiremock URI is http:// which is permitted in debug builds");

    // Assert that the TokenProvider error is surfaced without network I/O.
    let err = client.accounts().numbers().await.unwrap_err();
    assert!(
        matches!(err, Error::TokenProvider { .. }),
        "expected TokenProvider error, got: {err:?}"
    );
    assert!(
        !err.is_retryable(),
        "TokenProvider errors must not be retryable; consumer policy decides"
    );
    assert_eq!(provider.call_count(), 1);
}

#[tokio::test]
async fn schwab_client_new_uses_static_provider_under_the_hood() {
    // The same token is presented on every request.
    let trader = common::trader_mock().await;
    let market = common::market_mock().await;

    Mock::given(method("GET"))
        .and(path("/accounts/accountNumbers"))
        .and(header("Authorization", "Bearer static-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("accounts/numbers.json")),
        )
        .expect(2)
        .mount(&trader)
        .await;

    let client = SchwabClient::new(AuthToken::new("static-token"))
        .with_trader_base_url(trader.uri())
        .expect("wiremock URI is http:// which is permitted in debug builds")
        .with_market_data_base_url(market.uri())
        .expect("wiremock URI is http:// which is permitted in debug builds");

    client.accounts().numbers().await.unwrap();
    client.accounts().numbers().await.unwrap();
}
