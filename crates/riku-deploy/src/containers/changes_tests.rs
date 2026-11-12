use std::fs;

use crate::containers::mock_state::{deploy, mock_server_with_domain, MockServer};

/// A built, routed `web` and a pulled `redis` with a volume, as in the
/// two service apps people deploy.
fn web_and_redis(redis_image: &str, redis_host: &str) -> String {
    format!(
        "
services:
  web:
    build: ./web
    ports: [\"8000\"]
    environment:
      REDIS_HOST: {redis_host}
    depends_on: [redis]
    healthcheck:
      test: [\"CMD\", \"true\"]
  redis:
    image: {redis_image}
    volumes: [\"data:/data\"]
volumes:
  data: {{}}
"
    )
}

fn deployed_server(root: &std::path::Path) -> MockServer {
    let server = mock_server_with_domain(
        root,
        "myapp",
        &web_and_redis("redis:7-alpine", "redis"),
        "myapp.example.com",
    );
    fs::write(server.paths.env_root.join("myapp").join("ENV"), "FOO=1\n").unwrap();
    let web_dir = server.paths.app_root.join("myapp").join("web");
    fs::create_dir_all(&web_dir).unwrap();
    fs::write(web_dir.join("Dockerfile"), "FROM scratch\n").unwrap();
    deploy(&server).unwrap();
    server
}

fn set_compose(server: &MockServer, compose: &str) {
    let file = server.paths.app_root.join("myapp").join("compose.yaml");
    fs::write(file, compose).unwrap();
}

/// Services that got a new copy started, and whether anything was stopped,
/// in the calls made after `before`.
fn replaced(server: &MockServer, before: usize) -> (Vec<&'static str>, bool) {
    let calls = server.calls()[before..].to_vec();
    let started = ["web", "redis"]
        .into_iter()
        .filter(|service| {
            calls
                .iter()
                .any(|l| l.starts_with("run") && l.contains(&format!("myapp.{service}.")))
        })
        .collect();
    let stopped = calls.iter().any(|l| l.starts_with("stop"));
    (started, stopped)
}

#[test]
fn redeploy_with_nothing_changed_replaces_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let server = deployed_server(tmp.path());
    let before = server.calls().len();

    deploy(&server).unwrap();

    assert_eq!(
        replaced(&server, before),
        (vec![], false),
        "an unchanged redeploy replaced copies: {:#?}",
        &server.calls()[before..]
    );
    assert_eq!(server.containers(), ["myapp.redis.1", "myapp.web.1"]);
}

#[test]
fn kept_routed_service_still_gets_a_new_domain() {
    let tmp = tempfile::tempdir().unwrap();
    let server = deployed_server(tmp.path());
    fs::write(
        server.paths.app_settings_override("myapp"),
        "[routing]\ndomains = [\"other.example.com\"]\n",
    )
    .unwrap();
    let before = server.calls().len();

    deploy(&server).unwrap();

    assert_eq!(replaced(&server, before), (vec![], false));
    let site = server.site("myapp");
    assert!(
        site.contains("other.example.com"),
        "the new domain did not reach the router: {site}"
    );
}

#[test]
fn compose_change_to_web_replaces_web_and_keeps_redis() {
    let tmp = tempfile::tempdir().unwrap();
    let server = deployed_server(tmp.path());
    set_compose(&server, &web_and_redis("redis:7-alpine", "cache"));
    let before = server.calls().len();

    deploy(&server).unwrap();

    assert_eq!(
        replaced(&server, before).0,
        ["web"],
        "{:#?}",
        &server.calls()[before..]
    );
}

#[test]
fn image_change_replaces_that_service_only() {
    let tmp = tempfile::tempdir().unwrap();
    let server = deployed_server(tmp.path());
    set_compose(&server, &web_and_redis("redis:8-alpine", "redis"));
    let before = server.calls().len();

    deploy(&server).unwrap();

    assert_eq!(
        replaced(&server, before).0,
        ["redis"],
        "{:#?}",
        &server.calls()[before..]
    );
}

#[test]
fn new_image_id_under_the_same_tag_replaces_the_service() {
    let tmp = tempfile::tempdir().unwrap();
    let server = deployed_server(tmp.path());
    fs::write(server.state.join("image-id"), "sha256:rebuilt\n").unwrap();
    let before = server.calls().len();

    deploy(&server).unwrap();

    assert!(
        replaced(&server, before).0.contains(&"web"),
        "a rebuilt web image did not replace web: {:#?}",
        &server.calls()[before..]
    );
}

/// Only built services get the app env, so redis keeps running.
#[test]
fn app_env_change_replaces_web_and_not_redis() {
    let tmp = tempfile::tempdir().unwrap();
    let server = deployed_server(tmp.path());
    fs::write(server.paths.env_root.join("myapp").join("ENV"), "FOO=2\n").unwrap();
    let before = server.calls().len();

    deploy(&server).unwrap();

    assert_eq!(
        replaced(&server, before).0,
        ["web"],
        "{:#?}",
        &server.calls()[before..]
    );
}
