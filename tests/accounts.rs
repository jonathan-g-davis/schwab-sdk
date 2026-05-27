mod common;

use schwab_sdk::AccountHash;
use schwab_sdk::accounts::SecuritiesAccount;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn bearer() -> String {
    format!("Bearer {}", common::TEST_TOKEN)
}

#[tokio::test]
async fn numbers_returns_account_number_hashes() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);

    Mock::given(method("GET"))
        .and(path("/accounts/accountNumbers"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("accounts/numbers.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let hashes = client.accounts().numbers().await.unwrap();
    assert_eq!(hashes.len(), 1);
    assert_eq!(
        hashes[0].hash_value.expose_secret(),
        common::TEST_ACCOUNT_HASH
    );
}

#[tokio::test]
async fn list_without_positions_omits_fields_param() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);

    // Mounted first (lower priority); fires on a clean /accounts request.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("accounts/list.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    // Mounted second (higher LIFO priority); expect(0) fails the test if the
    // SDK sends ?fields=positions when it should not.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(query_param("fields", "positions"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .expect(0)
        .mount(&trader)
        .await;

    let accounts = client.accounts().list().send().await.unwrap();
    assert_eq!(accounts.len(), 1);
}

#[tokio::test]
async fn list_with_positions_sends_fields_param() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);

    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("Authorization", bearer()))
        .and(query_param("fields", "positions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("accounts/list.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let accounts = client
        .accounts()
        .list()
        .with_positions()
        .send()
        .await
        .unwrap();
    assert_eq!(accounts.len(), 1);
}

#[tokio::test]
async fn get_by_hash() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = AccountHash::new(common::TEST_ACCOUNT_HASH);

    Mock::given(method("GET"))
        .and(path(format!("/accounts/{}", common::TEST_ACCOUNT_HASH)))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("accounts/get.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let account = client.accounts().get(&hash).send().await.unwrap();
    assert_eq!(account.securities_account.account_type(), "CASH");
}

// The live API sends `isInCall` as a JSON boolean, but it is typed in the spec
// as a JSON number.
#[tokio::test]
async fn get_margin_parses_is_in_call_as_bool() {
    let trader = common::trader_mock().await;
    let client = common::client_for(&trader, &common::market_mock().await);
    let hash = AccountHash::new(common::TEST_ACCOUNT_HASH);

    Mock::given(method("GET"))
        .and(path(format!("/accounts/{}", common::TEST_ACCOUNT_HASH)))
        .and(header("Authorization", bearer()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(common::fixtures::read_fixture("accounts/get_margin.json")),
        )
        .expect(1)
        .mount(&trader)
        .await;

    let account = client.accounts().get(&hash).send().await.unwrap();
    let SecuritiesAccount::Margin(margin) = &account.securities_account else {
        panic!("expected a margin account");
    };
    assert_eq!(
        margin.initial_balances.as_ref().unwrap().is_in_call,
        Some(true)
    );
    assert_eq!(
        margin.current_balances.as_ref().unwrap().is_in_call,
        Some(false)
    );
}
