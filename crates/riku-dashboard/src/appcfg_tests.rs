use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use tempfile::TempDir;

use super::*;
use crate::config::RikuPaths;

fn state(tmp: &TempDir, token: Option<&str>) -> DashboardState {
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    std::fs::write(
        paths.env_root.join("shop").join("ENV"),
        "SECRET_KEY=hunter2\n",
    )
    .unwrap();
    DashboardState {
        paths,
        token: token.map(str::to_string),
    }
}

fn loopback() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, "127.0.0.1:8080".parse().unwrap());
    headers
}

async fn read_env(state: DashboardState, headers: HeaderMap) -> axum::response::Response {
    get_env(State(state), headers, Path("shop".to_string())).await
}

#[tokio::test]
async fn env_read_is_refused_without_a_token() {
    let tmp = TempDir::new().unwrap();

    let res = read_env(state(&tmp, None), loopback()).await;

    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "secrets served without a token"
    );
}

#[tokio::test]
async fn env_read_needs_the_bearer_token() {
    let tmp = TempDir::new().unwrap();
    let mut headers = loopback();

    let res = read_env(state(&tmp, Some("secret")), headers.clone()).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
    let res = read_env(state(&tmp, Some("secret")), headers).await;
    assert_eq!(res.status(), StatusCode::OK);
}
