use crate::common;

use schwab_sdk::market_data::{ContractType, OptionRange};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn get_chain_minimal() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/chains"))
        .and(query_param("symbol", "AAPL"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("chains/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let chain = client
        .market_data()
        .chains()
        .get("AAPL")
        .send()
        .await
        .unwrap();

    assert_eq!(chain.symbol.as_deref(), Some("AAPL"));
    assert_eq!(chain.status.as_deref(), Some("SUCCESS"));
    assert!(!chain.call_exp_date_map.is_empty());
    assert!(!chain.put_exp_date_map.is_empty());
}

#[tokio::test]
async fn get_chain_with_optional_params() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/chains"))
        .and(query_param("symbol", "AAPL"))
        .and(query_param("contractType", "CALL"))
        .and(query_param("strikeCount", "5"))
        .and(query_param("range", "NTM"))
        .and(query_param("includeUnderlyingQuote", "true"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("chains/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let chain = client
        .market_data()
        .chains()
        .get("AAPL")
        .contract_type(ContractType::Call)
        .strike_count(5)
        .range(OptionRange::Ntm)
        .include_underlying_quote(true)
        .send()
        .await
        .unwrap();

    assert_eq!(chain.symbol.as_deref(), Some("AAPL"));
    assert!(!chain.call_exp_date_map.is_empty());
}
