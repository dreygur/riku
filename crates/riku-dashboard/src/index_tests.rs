use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::router;
use crate::assets::ASSETS;
use crate::config::RikuPaths;
use crate::DashboardState;

/// Text only the embedded page carries, so the test fails if `/` serves
/// anything else.
const PAGE_MARKER: &str = r#"<script type="module" src="/assets/js/main.js">"#;

/// Spawns the dashboard with a token configured, so a 200 without one
/// proves the route is deliberately public rather than merely unconfigured.
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

/// Sends a token-less GET and returns the lowercased header block and body.
async fn get(addr: std::net::SocketAddr, path: &str) -> (String, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("send request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read response");
    let response = String::from_utf8(raw).expect("utf-8 response");
    let (head, body) = response
        .split_once("\r\n\r\n")
        .expect("response has a header block");
    (head.to_ascii_lowercase(), body.to_string())
}

#[tokio::test]
async fn root_serves_the_embedded_page_without_a_token() {
    let (head, body) = get(spawn_dashboard().await, "/").await;

    assert!(head.starts_with("http/1.1 200"), "status line: {head}");
    assert!(head.contains("content-type: text/html"), "headers: {head}");
    assert!(
        body.contains(PAGE_MARKER),
        "page body lacks {PAGE_MARKER:?}"
    );
}

#[tokio::test]
async fn every_asset_is_served_with_its_content_type_and_no_token_like_the_index() {
    let addr = spawn_dashboard().await;

    for (path, _, source) in ASSETS {
        let (head, body) = get(addr, &format!("/assets/{path}")).await;
        let expected = if path.ends_with(".css") {
            "content-type: text/css"
        } else {
            "content-type: text/javascript"
        };
        assert!(head.starts_with("http/1.1 200"), "{path} status: {head}");
        assert!(head.contains(expected), "{path} headers: {head}");
        assert_eq!(body, *source, "{path} body differs from the embedded file");
    }

    let (head, _) = get(addr, "/assets/../src/lib.rs").await;
    assert!(
        head.starts_with("http/1.1 404"),
        "unknown asset status: {head}"
    );
}

/// A module added under `assets/js` but left out of `ASSETS` would 404 in
/// the browser and break the page.
#[test]
fn asset_table_lists_every_module_on_disk() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut pending = vec![root.join("js")];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("read assets dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                if path.file_name() != Some("tests".as_ref()) {
                    pending.push(path);
                }
                continue;
            }
            let rel = path.strip_prefix(&root).expect("under assets");
            let rel = rel.to_string_lossy().replace('\\', "/");
            assert!(
                ASSETS.iter().any(|(name, _, _)| *name == rel),
                "{rel} is not in assets::ASSETS"
            );
        }
    }
}

/// The page runs on hosts reached over an ssh tunnel, often offline, so
/// neither it nor its assets may pull fonts or scripts from anywhere else.
#[tokio::test]
async fn page_and_assets_make_no_third_party_requests() {
    let (_, page) = get(spawn_dashboard().await, "/").await;
    let sources = std::iter::once(("index.html", page.as_str()))
        .chain(ASSETS.iter().map(|(path, _, body)| (*path, *body)));

    for (path, body) in sources {
        for needle in ["https://", "fonts.googleapis"] {
            assert!(!body.contains(needle), "{path} references {needle:?}");
        }
    }
}
