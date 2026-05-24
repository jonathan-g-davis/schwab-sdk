use crate::common;

use schwab_sdk::market_data::{InstrumentAssetType, Projection};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn search_instruments_by_symbol() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/instruments"))
        .and(query_param("symbol", "AAPL"))
        .and(query_param("projection", "symbol-search"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("instruments/list.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .instruments()
        .search("AAPL", Projection::SymbolSearch)
        .await
        .unwrap();

    assert_eq!(resp.instruments.len(), 2);
    assert_eq!(resp.instruments[0].symbol.as_deref(), Some("AAPL"));
    assert_eq!(
        resp.instruments[0].asset_type,
        Some(InstrumentAssetType::Equity)
    );
}

#[tokio::test]
async fn get_instrument_by_cusip() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/instruments/037833100"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("instruments/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let inst = client
        .market_data()
        .instruments()
        .get_by_cusip("037833100")
        .await
        .unwrap();

    assert_eq!(inst.symbol.as_deref(), Some("AAPL"));
    assert_eq!(inst.cusip.as_deref(), Some("037833100"));
    assert_eq!(inst.asset_type, Some(InstrumentAssetType::Equity));
}
