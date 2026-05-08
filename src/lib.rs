use std::sync::Arc;

use fastwebsockets::FragmentCollector;
use http::{Method, header::{CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE}};
use http_body_util::Empty;
use hyper::{Request, Uri, body::Bytes, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use rustls_platform_verifier::ConfigVerifierExt;
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, rustls};

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

pub async fn connect(uri: Uri) -> FragmentCollector<TokioIo<Upgraded>> {
    let host = uri.host().expect("host is required");
    let port = uri.port_u16().unwrap_or(443);
    let addr = format!("{}:{}", host, port);

    let socket = TcpStream::connect(addr).await.expect("failed to connect to server");
    let config = rustls::ClientConfig::with_platform_verifier().expect("failed to create client config");
    let connector = TlsConnector::from(Arc::new(config));

    let domain = rustls_pki_types::ServerName::try_from(host.to_string()).expect("invalid domain");
    let tls_stream = connector.connect(domain, socket).await.expect("failed to connect to TLS stream");

    let req = Request::builder()
        .method(Method::GET)
        .uri(uri.clone())
        .header(HOST, host)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(SEC_WEBSOCKET_KEY, fastwebsockets::handshake::generate_key())
        .header(SEC_WEBSOCKET_VERSION, "13")
        .body(Empty::<Bytes>::new())
        .expect("failed to build request");

    let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await.expect("failed to handshake");

    FragmentCollector::new(ws)
}
