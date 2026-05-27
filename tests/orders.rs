mod common;

use chrono::{TimeZone, Utc};
use rust_decimal_macros::dec;
use schwab_sdk::AccountHash;
use schwab_sdk::error::Error;
use schwab_sdk::orders::{ApiOrderStatus, OrderId, OrderRequest};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

fn account_hash() -> AccountHash {
    AccountHash::new(common::TEST_ACCOUNT_HASH)
}

fn test_order() -> impl Into<OrderRequest> {
    OrderRequest::buy_market("AAPL", dec!(1))
}

#[tokio::test]
async fn get() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("GET"))
        .and(path(format!(
            "/accounts/{}/orders/100000001",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("orders/get.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let order = client
        .orders(&hash)
        .get(OrderId::new(100000001))
        .await
        .unwrap();
    assert_eq!(order.order_id, Some(OrderId::new(100000001)));
}

#[tokio::test]
async fn list() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    let from = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2024, 1, 31, 23, 59, 59).unwrap();

    Mock::given(method("GET"))
        .and(path(format!(
            "/accounts/{}/orders",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .and(query_param("fromEnteredTime", "2024-01-01T00:00:00.000Z"))
        .and(query_param("toEnteredTime", "2024-01-31T23:59:59.000Z"))
        .and(query_param("maxResults", "10"))
        .and(query_param("status", "WORKING"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("orders/list.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let orders = client
        .orders(&hash)
        .list(from, to)
        .max_results(10)
        .status(ApiOrderStatus::Working)
        .send()
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].order_id, Some(OrderId::new(100000001)));
}

#[tokio::test]
async fn place_returns_order_id_from_location_header() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("POST"))
        .and(path(format!(
            "/accounts/{}/orders",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(ResponseTemplate::new(201).insert_header(
            "Location",
            format!("/accounts/{}/orders/100000001", common::TEST_ACCOUNT_HASH),
        ))
        .expect(1)
        .mount(&trader)
        .await;

    let id = client.orders(&hash).place(test_order()).await.unwrap();
    assert_eq!(id, OrderId::new(100000001));
}

#[tokio::test]
async fn place_missing_location_header_returns_error() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("POST"))
        .and(path(format!(
            "/accounts/{}/orders",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&trader)
        .await;

    let err = client.orders(&hash).place(test_order()).await.unwrap_err();
    assert!(matches!(err, Error::OrderIdUnrecoverable(_)));
}

#[tokio::test]
async fn replace_returns_new_order_id() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("PUT"))
        .and(path(format!(
            "/accounts/{}/orders/100000001",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(ResponseTemplate::new(200).insert_header(
            "Location",
            format!("/accounts/{}/orders/100000002", common::TEST_ACCOUNT_HASH),
        ))
        .expect(1)
        .mount(&trader)
        .await;

    let id = client
        .orders(&hash)
        .replace(OrderId::new(100000001), test_order())
        .await
        .unwrap();
    assert_eq!(id, OrderId::new(100000002));
}

#[tokio::test]
async fn replace_missing_location_header_returns_error() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("PUT"))
        .and(path(format!(
            "/accounts/{}/orders/100000001",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&trader)
        .await;

    let err = client
        .orders(&hash)
        .replace(OrderId::new(100000001), test_order())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::OrderIdUnrecoverable(_)));
}

#[tokio::test]
async fn cancel() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("DELETE"))
        .and(path(format!(
            "/accounts/{}/orders/100000001",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&trader)
        .await;

    client
        .orders(&hash)
        .cancel(OrderId::new(100000001))
        .await
        .unwrap();
}

#[tokio::test]
async fn preview() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("POST"))
        .and(path(format!(
            "/accounts/{}/previewOrder",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("orders/preview.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let preview = client.orders(&hash).preview(test_order()).await.unwrap();
    assert_eq!(preview.order_id, Some(999999999));
}

#[tokio::test]
async fn list_all_cross_account() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);

    let from = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2024, 1, 31, 23, 59, 59).unwrap();

    Mock::given(method("GET"))
        .and(path("/orders"))
        .and(header("Authorization", bearer()))
        .and(query_param("fromEnteredTime", "2024-01-01T00:00:00.000Z"))
        .and(query_param("toEnteredTime", "2024-01-31T23:59:59.000Z"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("orders/list.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let orders = client.orders_all().list(from, to).send().await.unwrap();
    assert_eq!(orders.len(), 1);
}
