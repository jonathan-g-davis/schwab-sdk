use std::sync::Arc;

use http::{
    Method,
    header::{CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
};
use http_body_util::Empty;
use hyper::{Request, Uri, body::Bytes, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use rustls_platform_verifier::ConfigVerifierExt;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream, rustls};

pub(crate) type WebSocket = fastwebsockets::WebSocket<TokioIo<Upgraded>>;

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

#[derive(Debug, Error)]
pub enum WebSocketError {
    #[error("failed to connect to server")]
    Connect(std::io::Error),
    #[error("failed to perform websocket handshake")]
    Handshake(fastwebsockets::WebSocketError),
    #[error("invalid domain")]
    InvalidDomain(rustls_pki_types::InvalidDnsNameError),
    #[error("host is required")]
    MissingHost,
    #[error("failed to create TLS stream")]
    TlsStream(std::io::Error),
}

async fn connect_tls(uri: &Uri) -> Result<TlsStream<TcpStream>, WebSocketError> {
    let host = uri.host().ok_or(WebSocketError::MissingHost)?;
    let port = uri.port_u16().unwrap_or(443);
    let addr = format!("{}:{}", host, port);

    // Connect to the server
    let socket = TcpStream::connect(addr)
        .await
        .map_err(WebSocketError::Connect)?;

    // Perform the TLS handshake and return the TLS stream
    let domain = rustls_pki_types::ServerName::try_from(host.to_string())
        .map_err(WebSocketError::InvalidDomain)?;
    let config = rustls::ClientConfig::with_platform_verifier()
        .expect("failed to create client config from platform verifier");
    let connector = TlsConnector::from(Arc::new(config));
    connector
        .connect(domain, socket)
        .await
        .map_err(WebSocketError::TlsStream)
}

pub(crate) async fn connect(uri: Uri) -> Result<WebSocket, WebSocketError> {
    let tls_stream = connect_tls(&uri).await?;

    // Build the websocket upgrade request
    let req = Request::builder()
        .method(Method::GET)
        .uri(&uri)
        .header(HOST, uri.host().ok_or(WebSocketError::MissingHost)?)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(SEC_WEBSOCKET_KEY, fastwebsockets::handshake::generate_key())
        .header(SEC_WEBSOCKET_VERSION, "13")
        .body(Empty::<Bytes>::new())
        .expect("failed to build request");

    // Perform the websocket handshake and return the websocket stream
    let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream)
        .await
        .map_err(WebSocketError::Handshake)?;

    Ok(ws)
}
