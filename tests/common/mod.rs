// Shared across multiple integration-test binaries; dead_code fires in any
// binary that doesn't use every helper.
#![allow(dead_code)]

pub mod fixtures;

use schwab_sdk::{AuthToken, SchwabClient};
use wiremock::MockServer;

pub const TEST_TOKEN: &str = "test-bearer-token";
pub const TEST_ACCOUNT_HASH: &str = "ABC123HASH";

pub async fn trader_mock() -> MockServer {
    MockServer::start().await
}

pub async fn market_mock() -> MockServer {
    MockServer::start().await
}

pub fn client_for(trader: &MockServer, market: &MockServer) -> SchwabClient {
    SchwabClient::new(AuthToken::new(TEST_TOKEN))
        .with_trader_base_url(trader.uri())
        .with_market_data_base_url(market.uri())
}
