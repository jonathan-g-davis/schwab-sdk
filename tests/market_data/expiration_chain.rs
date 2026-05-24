use crate::common;

use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn get_expiration_chain() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/expirationchain"))
        .and(query_param("symbol", "AAPL"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("expiration_chain/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .expiration_chain()
        .get("AAPL")
        .await
        .unwrap();

    assert_eq!(resp.status.as_deref(), Some("SUCCESS"));
    assert_eq!(resp.expiration_list.len(), 3);
    assert_eq!(
        resp.expiration_list[0].expiration_date.as_deref(),
        Some("2024-03-15")
    );
    assert_eq!(resp.expiration_list[0].days_to_expiration, Some(1));
    assert_eq!(resp.expiration_list[0].standard, Some(true));
}
