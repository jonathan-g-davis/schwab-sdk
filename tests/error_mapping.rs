mod common;

use std::time::Duration;

use schwab_sdk::error::{Error, ErrorBody};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

/// Mount a GET /accounts mock on `trader` that returns the given response.
async fn mount(trader: &wiremock::MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("Authorization", bearer()))
        .respond_with(response)
        .expect(1)
        .mount(trader)
        .await;
}

#[tokio::test]
async fn unauthorized_trader_error_body() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    mount(
        &trader,
        ResponseTemplate::new(401)
            .set_body_string(common::fixtures::read_fixture("errors/trader_error.json")),
    )
    .await;

    let err = client.accounts().list().send().await.unwrap_err();
    assert!(matches!(err, Error::Unauthorized(ErrorBody::Trader(_))));
}

#[tokio::test]
async fn not_found_market_data_error_body() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    mount(
        &trader,
        ResponseTemplate::new(404).set_body_string(common::fixtures::read_fixture(
            "errors/market_data_error.json",
        )),
    )
    .await;

    let err = client.accounts().list().send().await.unwrap_err();
    assert!(matches!(err, Error::NotFound(ErrorBody::MarketData(_))));
}

#[tokio::test]
async fn rate_limited_with_retry_after() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    mount(
        &trader,
        ResponseTemplate::new(429)
            .set_body_string(common::fixtures::read_fixture("errors/trader_error.json"))
            .insert_header("Retry-After", "30"),
    )
    .await;

    let err = client.accounts().list().send().await.unwrap_err();
    assert!(
        matches!(err, Error::RateLimited { retry_after: Some(d), .. } if d == Duration::from_secs(30))
    );
}

#[tokio::test]
async fn rate_limited_without_retry_after() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    mount(
        &trader,
        ResponseTemplate::new(429)
            .set_body_string(common::fixtures::read_fixture("errors/trader_error.json")),
    )
    .await;

    let err = client.accounts().list().send().await.unwrap_err();
    assert!(matches!(
        err,
        Error::RateLimited {
            retry_after: None,
            ..
        }
    ));
}

#[tokio::test]
async fn http_500_trader_body() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    mount(
        &trader,
        ResponseTemplate::new(500)
            .set_body_string(common::fixtures::read_fixture("errors/trader_error.json")),
    )
    .await;

    let err = client.accounts().list().send().await.unwrap_err();
    match err {
        Error::Http {
            status,
            body: ErrorBody::Trader(_),
            ..
        } => assert_eq!(status.as_u16(), 500),
        other => panic!("expected Http {{ status: 500, body: Trader(_) }}, got {other:?}"),
    }
}

#[tokio::test]
async fn http_503_unrecognized_body() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    mount(
        &trader,
        ResponseTemplate::new(503)
            .set_body_string(common::fixtures::read_fixture("errors/unrecognized.txt")),
    )
    .await;

    let err = client.accounts().list().send().await.unwrap_err();
    match err {
        Error::Http {
            status,
            body: ErrorBody::Unrecognized(_),
            ..
        } => assert_eq!(status.as_u16(), 503),
        other => panic!("expected Http {{ status: 503, body: Unrecognized(_) }}, got {other:?}"),
    }
}

#[tokio::test]
async fn codec_error_on_malformed_json() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    mount(
        &trader,
        ResponseTemplate::new(200).set_body_string("{not valid json}"),
    )
    .await;

    let err = client.accounts().list().send().await.unwrap_err();
    assert!(matches!(err, Error::Codec { .. }));
}
