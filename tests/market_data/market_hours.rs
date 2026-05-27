use crate::common;

use schwab_sdk::market_data::Market;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn get_market_hours_single_market() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/markets/equity"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("market_hours/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .market_hours()
        .get(Market::Equity)
        .send()
        .await
        .unwrap();

    let equity = resp.get("equity").unwrap();
    let eq = equity.get("EQ").unwrap();
    assert_eq!(eq.is_open, Some(true));
    assert_eq!(eq.date.as_deref(), Some("2024-03-14"));
    assert!(eq.session_hours.contains_key("regularMarket"));
}

#[tokio::test]
async fn list_market_hours_multi_market() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/markets"))
        .and(query_param("markets", "equity,bond"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("market_hours/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .market_hours()
        .list([Market::Equity, Market::Bond])
        .send()
        .await
        .unwrap();

    assert!(resp.contains_key("equity"));
}
