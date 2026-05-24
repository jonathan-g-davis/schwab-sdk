use crate::common;

use schwab_sdk::market_data::QuoteEntry;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn list_quotes_returns_map_keyed_by_symbol() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/quotes"))
        .and(query_param("symbols", "AAPL,MSFT"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("quotes/list.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .quotes()
        .list(["AAPL", "MSFT"])
        .send()
        .await
        .unwrap();

    assert_eq!(resp.len(), 2);
    let aapl = resp.get("AAPL").unwrap();
    assert!(matches!(aapl, QuoteEntry::Equity(_)));
    if let QuoteEntry::Equity(q) = aapl {
        assert_eq!(q.symbol.as_deref(), Some("AAPL"));
    }
}

#[tokio::test]
async fn get_quote_hits_symbol_quotes_path() {
    let market = common::market_mock().await;
    let client = common::client_for(&common::trader_mock().await, &market);

    Mock::given(method("GET"))
        .and(path("/AAPL/quotes"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("quotes/get.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    let resp = client
        .market_data()
        .quotes()
        .get("AAPL")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.len(), 1);
    let entry = resp.get("AAPL").unwrap();
    assert!(matches!(entry, QuoteEntry::Equity(_)));
    if let QuoteEntry::Equity(q) = entry {
        assert_eq!(q.symbol.as_deref(), Some("AAPL"));
        let quote = q.quote.as_ref().unwrap();
        assert!(quote.last_price.is_some());
    }
}

#[tokio::test]
async fn list_quotes_hits_market_data_base_url_not_trader() {
    // Mounting the expectation on the trader mock (wrong base URL) would leave
    // it unmatched and verify-on-drop would catch it. Here we confirm it hits
    // the market mock by mounting expect(1) there.
    let trader = common::trader_mock().await;
    let market = common::market_mock().await;
    let client = common::client_for(&trader, &market);

    Mock::given(method("GET"))
        .and(path("/quotes"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("quotes/list.json")),
        )
        .expect(1)
        .mount(&market)
        .await;

    // expect(0) on trader - if SDK accidentally hits trader base URL, this fires
    Mock::given(method("GET"))
        .and(path("/quotes"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&trader)
        .await;

    client
        .market_data()
        .quotes()
        .list(["AAPL", "MSFT"])
        .send()
        .await
        .unwrap();
}
