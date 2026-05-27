mod common;

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn get() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);

    Mock::given(method("GET"))
        .and(path("/userPreference"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("user_preferences/get.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let prefs = client.user_preferences().get().await.unwrap();
    assert!(prefs.accounts[0].primary_account);
    assert_eq!(
        prefs.streamer_info[0].streamer_socket_url.as_deref(),
        Some("wss://streamer-api.schwab.com/ws"),
    );
}
