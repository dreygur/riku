use axum::extract::{Json, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use tempfile::TempDir;

use super::*;
use crate::config::RikuPaths;
use crate::DashboardState;

fn test_state(paths: RikuPaths, token: Option<&str>) -> DashboardState {
    DashboardState {
        paths,
        token: token.map(str::to_string),
    }
}

fn auth_header(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    h
}

#[tokio::test]
async fn service_restart_needs_token() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let st = test_state(paths, Some("secret"));
    let res = restart(
        State(st),
        HeaderMap::new(),
        Path(("edge".to_string(), "web".to_string())),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bad_service_name_is_400() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let st = test_state(paths, Some("secret"));
    let headers = auth_header("secret");
    let res = restart(
        State(st),
        headers,
        Path(("edge".to_string(), "-invalid-name".to_string())),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn scale_calls_scale_service() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let st = test_state(paths, Some("secret"));
    let headers = auth_header("secret");
    let body = serde_json::json!({ "copies": 2 });
    let res = scale(
        State(st),
        headers,
        Path(("edge".to_string(), "web".to_string())),
        Json(body),
    )
    .await;
    assert_ne!(res.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(res.status(), StatusCode::BAD_REQUEST);
}
