use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::router;
use crate::config::RikuPaths;
use crate::DashboardState;

/// Text only the embedded page carries, so the test fails if `/` serves
/// anything else.
const PAGE_MARKER: &str = "Nothing deployed yet";

const REQUEST: &[u8] = b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";

async fn spawn_dashboard() -> std::net::SocketAddr {
    let state = DashboardState {
        paths: RikuPaths::from_dirs("/tmp/riku-index-test".into(), std::path::Path::new("/tmp")),
        token: Some("secret".to_string()),
    };
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("listener address");
    tokio::spawn(async move { axum::serve(listener, router(state)).await });
    addr
}

async fn get_root(addr: std::net::SocketAddr) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream.write_all(REQUEST).await.expect("send request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read response");
    String::from_utf8(raw).expect("utf-8 response")
}

#[tokio::test]
async fn root_serves_the_embedded_page_without_a_token() {
    let response = get_root(spawn_dashboard().await).await;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .expect("response has a header block");
    let head = head.to_ascii_lowercase();

    assert!(head.starts_with("http/1.1 200"), "status line: {head}");
    assert!(head.contains("content-type: text/html"), "headers: {head}");
    assert!(
        body.contains(PAGE_MARKER),
        "page body lacks {PAGE_MARKER:?}"
    );
}
