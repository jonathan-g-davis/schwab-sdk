use crate::common;

use schwab_sdk::market_data::{MoverIndex, MoverSort};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn get_movers_minimal() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/movers/$DJI"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("movers/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .movers()
        .get(MoverIndex::Dji)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.screeners.len(), 2);
    assert_eq!(resp.screeners[0].symbol.as_deref(), Some("NVDA"));
    assert!(resp.screeners[0].last.is_some());
}

#[tokio::test]
async fn get_movers_with_sort_and_frequency() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/movers/$SPX"))
        .and(query_param("sort", "VOLUME"))
        .and(query_param("frequency", "5"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("movers/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .movers()
        .get(MoverIndex::Spx)
        .sort(MoverSort::Volume)
        .frequency(5)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.screeners.len(), 2);
}
