use crate::common;

use schwab_sdk::market_data::{FrequencyType, PeriodType};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn get_price_history_minimal() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/pricehistory"))
        .and(query_param("symbol", "AAPL"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("price_history/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let candles = client
        .market_data()
        .price_history()
        .get("AAPL")
        .send()
        .await
        .unwrap();

    assert_eq!(candles.symbol.as_deref(), Some("AAPL"));
    assert!(!candles.empty);
    assert_eq!(candles.candles.len(), 2);
    assert!(candles.candles[0].close.is_some());
}

#[tokio::test]
async fn get_price_history_full_optional_params() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/pricehistory"))
        .and(query_param("symbol", "AAPL"))
        .and(query_param("periodType", "day"))
        .and(query_param("period", "5"))
        .and(query_param("frequencyType", "minute"))
        .and(query_param("frequency", "1"))
        .and(query_param("needExtendedHoursData", "false"))
        .and(query_param("needPreviousClose", "true"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("price_history/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let candles = client
        .market_data()
        .price_history()
        .get("AAPL")
        .period_type(PeriodType::Day)
        .period(5)
        .frequency_type(FrequencyType::Minute)
        .frequency(1)
        .need_extended_hours_data(false)
        .need_previous_close(true)
        .send()
        .await
        .unwrap();

    assert_eq!(candles.symbol.as_deref(), Some("AAPL"));
    assert_eq!(candles.candles.len(), 2);
    assert!(candles.previous_close.is_some());
}
