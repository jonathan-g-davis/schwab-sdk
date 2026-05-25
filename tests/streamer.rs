//! Streamer integration test: drive `streamer::connect` against a scripted
//! `tokio-tungstenite` server on `127.0.0.1:0` to validate the on-the-wire
//! login + subscribe + data + logout sequence and the `notify` heartbeat
//! decode. Exercises the `ws://` dispatch path added to `connect_websocket`.

mod common;

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use common::TEST_TOKEN;
use futures_util::{SinkExt, StreamExt};
use rust_decimal_macros::dec;
use schwab_sdk::error::Error;
use schwab_sdk::streamer::{
    self, DataContent, Service, StreamerCommand, StreamerResponse, SubscriptionCommand,
    level_one::equities::Field,
};
use schwab_sdk::user_preferences::StreamerInfo;
use schwab_sdk::{AuthToken, StaticTokenProvider, TokenProvider};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Bind a TCP listener on an ephemeral port and spawn `handler` to drive the
/// scripted server side of one WebSocket upgrade. Returns the bound address
/// for the client to point at, and the spawned task so the test can `await`
/// it to surface panics raised inside the handler.
async fn spawn_ws_server<F, Fut>(handler: F) -> (SocketAddr, JoinHandle<()>)
where
    F: FnOnce(WebSocketStream<TcpStream>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.expect("accept");
        let ws = tokio_tungstenite::accept_async(tcp)
            .await
            .expect("websocket handshake");
        handler(ws).await;
    });
    (addr, server)
}

/// Build a `StreamerInfo` pointing at the in-process server. Round-tripped
/// through serde because `StreamerInfo` is `#[non_exhaustive]` and so can't
/// be struct-literal-constructed from outside the crate.
fn streamer_info_for(addr: SocketAddr) -> StreamerInfo {
    serde_json::from_value(json!({
        "streamerSocketUrl": format!("ws://{addr}/ws"),
        "schwabClientCustomerId": "test-cust",
        "schwabClientCorrelId": "test-corr",
        "schwabClientChannel": "N9",
        "schwabClientFunctionId": "APIAPP",
    }))
    .expect("StreamerInfo deserialize")
}

/// Block until the server reads one text frame (skipping any ping/pong
/// control frames) and parse it as JSON. Times out after `TEST_TIMEOUT`.
async fn expect_text_frame(ws: &mut WebSocketStream<TcpStream>) -> Value {
    loop {
        let msg = timeout(TEST_TIMEOUT, ws.next())
            .await
            .expect("timeout waiting for client frame")
            .expect("server stream ended unexpectedly")
            .expect("frame read error");
        match msg {
            Message::Text(t) => {
                return serde_json::from_str(t.as_str()).expect("frame is not valid JSON");
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected non-text frame from client: {other:?}"),
        }
    }
}

async fn send_text(ws: &mut WebSocketStream<TcpStream>, value: Value) {
    ws.send(Message::text(value.to_string()))
        .await
        .expect("send text frame");
}

/// Build a `StaticTokenProvider` wrapping `token`, type-erased to the
/// `Arc<dyn TokenProvider + Send + Sync>` shape `streamer::connect`
/// expects.
fn static_provider(token: &str) -> Arc<dyn TokenProvider + Send + Sync> {
    Arc::new(StaticTokenProvider::new(AuthToken::new(token)))
}

#[tokio::test]
async fn login_subscribe_recv_logout() {
    let (addr, server) = spawn_ws_server(|mut ws| async move {
        // 1. Login frame.
        let login = expect_text_frame(&mut ws).await;
        assert_eq!(login["service"], "ADMIN");
        assert_eq!(login["command"], "LOGIN");
        assert_eq!(login["parameters"]["Authorization"], TEST_TOKEN);
        assert_eq!(login["parameters"]["SchwabClientChannel"], "N9");
        assert_eq!(login["parameters"]["SchwabClientFunctionId"], "APIAPP");
        assert_eq!(login["SchwabClientCustomerId"], "test-cust");
        assert_eq!(login["SchwabClientCorrelId"], "test-corr");

        // 2. Login success response.
        send_text(
            &mut ws,
            json!({
                "response": [{
                    "service": "ADMIN",
                    "command": "LOGIN",
                    "requestid": login["requestid"],
                    "SchwabClientCorrelId": "test-corr",
                    "timestamp": 1,
                    "content": { "code": 0, "msg": "server=test;status=PN" }
                }]
            }),
        )
        .await;

        // 3. Subscribe frame.
        let subs = expect_text_frame(&mut ws).await;
        assert_eq!(subs["service"], "LEVELONE_EQUITIES");
        assert_eq!(subs["command"], "SUBS");
        assert_eq!(subs["parameters"]["keys"], "AAPL");
        let fields_csv = subs["parameters"]["fields"]
            .as_str()
            .expect("fields csv string")
            .to_string();
        let mut parts: Vec<&str> = fields_csv.split(',').collect();
        parts.sort();
        // BidPrice=1, AskPrice=2 by the equities `Field` repr; wire form is csv.
        assert_eq!(parts, vec!["1", "2"]);

        // 4. Data frame with a `LevelOneEquities` content block.
        send_text(
            &mut ws,
            json!({
                "data": [{
                    "service": "LEVELONE_EQUITIES",
                    "timestamp": 2,
                    "command": "SUBS",
                    "content": [{
                        "key": "AAPL",
                        "delayed": false,
                        "1": 183.75,
                        "2": 183.80
                    }]
                }]
            }),
        )
        .await;

        // 5. Logout frame.
        let logout = expect_text_frame(&mut ws).await;
        assert_eq!(logout["service"], "ADMIN");
        assert_eq!(logout["command"], "LOGOUT");

        // 6. Logout response, then clean close.
        send_text(
            &mut ws,
            json!({
                "response": [{
                    "service": "ADMIN",
                    "command": "LOGOUT",
                    "requestid": logout["requestid"],
                    "SchwabClientCorrelId": "test-corr",
                    "timestamp": 3,
                    "content": { "code": 0, "msg": "" }
                }]
            }),
        )
        .await;
        ws.close(None).await.expect("close");
    })
    .await;

    let (mut reader, writer) =
        streamer::connect(streamer_info_for(addr), static_provider(TEST_TOKEN))
            .await
            .expect("connect");

    writer.login().await.expect("login send");

    let login_resp = timeout(TEST_TIMEOUT, reader.recv())
        .await
        .expect("login response timed out")
        .expect("login response");
    let StreamerResponse::Response(responses) = login_resp else {
        panic!("expected Response after login, got {login_resp:?}");
    };
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].service, Service::Admin);
    assert_eq!(responses[0].command, StreamerCommand::Login);

    writer
        .equities()
        .subscribe(["AAPL"])
        .fields([Field::BidPrice, Field::AskPrice])
        .send()
        .await
        .expect("subscribe send");

    let data = timeout(TEST_TIMEOUT, reader.recv())
        .await
        .expect("data frame timed out")
        .expect("data frame");
    let StreamerResponse::Data(payloads) = data else {
        panic!("expected Data, got {data:?}");
    };
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].service, Service::LevelOneEquities);
    assert_eq!(payloads[0].command, SubscriptionCommand::Subscribe);
    let DataContent::LevelOneEquities(items) = &payloads[0].content else {
        panic!(
            "expected LevelOneEquities content, got {:?}",
            payloads[0].content
        );
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "AAPL");
    assert_eq!(items[0].bid_price, Some(dec!(183.75)));
    assert_eq!(items[0].ask_price, Some(dec!(183.80)));

    writer.logout().await.expect("logout send");
    let logout_resp = timeout(TEST_TIMEOUT, reader.recv())
        .await
        .expect("logout response timed out")
        .expect("logout response");
    let StreamerResponse::Response(responses) = logout_resp else {
        panic!("expected Response after logout, got {logout_resp:?}");
    };
    assert_eq!(responses[0].command, StreamerCommand::Logout);

    server.await.expect("server task");
}

#[tokio::test]
async fn heartbeat_notify_decodes() {
    let (addr, server) = spawn_ws_server(|mut ws| async move {
        // Drain the login frame so the dance is realistic (a heartbeat can
        // arrive at any point in an established session).
        let login = expect_text_frame(&mut ws).await;
        assert_eq!(login["service"], "ADMIN");
        assert_eq!(login["command"], "LOGIN");
        send_text(
            &mut ws,
            json!({
                "response": [{
                    "service": "ADMIN",
                    "command": "LOGIN",
                    "requestid": login["requestid"],
                    "SchwabClientCorrelId": "test-corr",
                    "timestamp": 1,
                    "content": { "code": 0, "msg": "ok" }
                }]
            }),
        )
        .await;

        // The frame under test.
        send_text(
            &mut ws,
            json!({ "notify": [{ "heartbeat": "1668715930582" }] }),
        )
        .await;

        // Hold the socket open until the client drops, swallowing stray
        // frames so the spawned task exits cleanly.
        while let Some(msg) = ws.next().await {
            if matches!(msg, Ok(Message::Close(_)) | Err(_)) {
                break;
            }
        }
    })
    .await;

    let (mut reader, writer) =
        streamer::connect(streamer_info_for(addr), static_provider(TEST_TOKEN))
            .await
            .expect("connect");
    writer.login().await.expect("login send");

    // Drain the login response so the next frame is the notify.
    let _login_resp = timeout(TEST_TIMEOUT, reader.recv())
        .await
        .expect("login response timed out")
        .expect("login response");

    let notify = timeout(TEST_TIMEOUT, reader.recv())
        .await
        .expect("notify timed out")
        .expect("notify");
    let StreamerResponse::Notify(heartbeats) = notify else {
        panic!("expected Notify, got {notify:?}");
    };
    assert_eq!(heartbeats.len(), 1);
    assert_eq!(heartbeats[0].heartbeat, 1_668_715_930_582);

    drop(reader);
    drop(writer);
    let _ = server.await;
}

/// `TokenProvider` that serves a scripted sequence of tokens, advancing
/// one step per `access_token` call. Used to assert that the streamer
/// re-reads the provider on every `login`, so a token rotated between
/// the initial LOGIN and a re-LOGIN is the value carried on the wire.
struct ScriptedProvider(Mutex<std::vec::IntoIter<&'static str>>);

impl ScriptedProvider {
    fn new(tokens: Vec<&'static str>) -> Arc<Self> {
        Arc::new(Self(Mutex::new(tokens.into_iter())))
    }
}

#[async_trait]
impl TokenProvider for ScriptedProvider {
    async fn access_token(&self) -> Result<AuthToken, Error> {
        let next = self
            .0
            .lock()
            .unwrap()
            .next()
            .expect("scripted tokens exhausted");
        Ok(AuthToken::new(next))
    }
}

#[tokio::test]
async fn login_re_reads_provider_so_rotation_propagates_to_the_wire() {
    let (addr, server) = spawn_ws_server(|mut ws| async move {
        // First LOGIN carries token A.
        let first = expect_text_frame(&mut ws).await;
        assert_eq!(first["service"], "ADMIN");
        assert_eq!(first["command"], "LOGIN");
        assert_eq!(first["parameters"]["Authorization"], "token-A");
        send_text(
            &mut ws,
            json!({
                "response": [{
                    "service": "ADMIN",
                    "command": "LOGIN",
                    "requestid": first["requestid"],
                    "SchwabClientCorrelId": "test-corr",
                    "timestamp": 1,
                    "content": { "code": 0, "msg": "ok" }
                }]
            }),
        )
        .await;

        // Second LOGIN (re-LOGIN) must carry token B - the provider has
        // rotated between the two calls, and the streamer must re-fetch
        // rather than cache the first value.
        let second = expect_text_frame(&mut ws).await;
        assert_eq!(second["service"], "ADMIN");
        assert_eq!(second["command"], "LOGIN");
        assert_eq!(second["parameters"]["Authorization"], "token-B");
        send_text(
            &mut ws,
            json!({
                "response": [{
                    "service": "ADMIN",
                    "command": "LOGIN",
                    "requestid": second["requestid"],
                    "SchwabClientCorrelId": "test-corr",
                    "timestamp": 2,
                    "content": { "code": 0, "msg": "ok" }
                }]
            }),
        )
        .await;

        while let Some(msg) = ws.next().await {
            if matches!(msg, Ok(Message::Close(_)) | Err(_)) {
                break;
            }
        }
    })
    .await;

    let provider = ScriptedProvider::new(vec!["token-A", "token-B"]);
    let (mut reader, writer) = streamer::connect(streamer_info_for(addr), provider)
        .await
        .expect("connect");

    writer.login().await.expect("first login send");
    let _first = timeout(TEST_TIMEOUT, reader.recv())
        .await
        .expect("first login response timed out")
        .expect("first login response");

    // Re-LOGIN: the provider advances to token-B, which the server-side
    // assertion above verifies arrives on the wire.
    writer.login().await.expect("re-login send");
    let _second = timeout(TEST_TIMEOUT, reader.recv())
        .await
        .expect("re-login response timed out")
        .expect("re-login response");

    drop(reader);
    drop(writer);
    let _ = server.await;
}
