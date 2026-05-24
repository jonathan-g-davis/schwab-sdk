mod common;

use chrono::{TimeZone, Utc};
use schwab_sdk::AccountHash;
use schwab_sdk::transactions::TransactionType;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

fn account_hash() -> AccountHash {
    AccountHash::new(common::TEST_ACCOUNT_HASH)
}

#[tokio::test]
async fn list_serializes_dates_as_rfc3339_millis() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2024, 3, 31, 23, 59, 59).unwrap();

    Mock::given(method("GET"))
        .and(path(format!(
            "/accounts/{}/transactions",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .and(query_param("startDate", "2024-01-01T00:00:00.000Z"))
        .and(query_param("endDate", "2024-03-31T23:59:59.000Z"))
        .and(query_param("types", "TRADE"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("transactions/list.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let txns = client
        .transactions(&hash)
        .list(start, end, TransactionType::Trade)
        .send()
        .await
        .unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].activity_id, Some(9876543210i64));
}

#[tokio::test]
async fn get_by_id() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = account_hash();

    Mock::given(method("GET"))
        .and(path(format!(
            "/accounts/{}/transactions/1111111111",
            common::TEST_ACCOUNT_HASH
        )))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("transactions/get.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let txns = client.transactions(&hash).get(1111111111).await.unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].activity_id, Some(1111111111i64));
}
