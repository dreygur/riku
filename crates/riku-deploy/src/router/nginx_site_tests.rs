use std::cell::RefCell;
use std::fs;
use std::process::{Command, Stdio};

use riku_route::Stage;

use super::*;
use crate::router::app_routing::RoutingMode;
use crate::util::route_plan::HttpsPolicy;

fn mock_helper(
    reply: Reply,
    seen: &RefCell<Vec<Request>>,
) -> impl Fn(&Request) -> Result<Reply> + '_ {
    move |request| {
        seen.borrow_mut().push(request.clone());
        Ok(reply.clone())
    }
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn setup() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.nginx_root).unwrap();
    fs::create_dir_all(&paths.static_root).unwrap();
    fs::create_dir_all(&paths.socket_root).unwrap();
    (tmp, paths)
}

fn apply(
    send: impl Fn(&Request) -> Result<Reply>,
    env: &HashMap<String, String>,
    tmp: &tempfile::TempDir,
    paths: &RikuPaths,
) -> Result<()> {
    apply_routed(send, env, &own_domains(), tmp, paths)
}

fn own_domains() -> AppRouting {
    AppRouting {
        domains: Vec::new(),
        fallback: false,
        port: None,
        mode: RoutingMode::Domain,
        legacy: true,
        fallback_skipped: None,
    }
}

fn apply_routed(
    send: impl Fn(&Request) -> Result<Reply>,
    env: &HashMap<String, String>,
    routing: &AppRouting,
    tmp: &tempfile::TempDir,
    paths: &RikuPaths,
) -> Result<()> {
    apply_nginx_site(
        send,
        "myapp",
        tmp.path(),
        env,
        routing,
        &[Backend::loopback(5000)],
        paths,
    )
}

fn applied(request: &Request) -> &riku_route::Route {
    match request {
        Request::Apply { route, .. } => route,
        other => panic!("expected an apply, got {other:?}"),
    }
}

#[test]
fn apply_sends_the_route_and_a_refusal_names_its_stage() {
    let (tmp, paths) = setup();
    let seen = RefCell::new(Vec::new());
    let env = env(&[("RIKU_DOMAINS", "myapp.example")]);

    apply(
        mock_helper(Reply::Ok { reloaded: true }, &seen),
        &env,
        &tmp,
        &paths,
    )
    .unwrap();
    let refused = Reply::Failed {
        stage: Stage::Check,
        error: "emerg: unknown directive".to_string(),
    };
    let err = apply(mock_helper(refused, &seen), &env, &tmp, &paths)
        .unwrap_err()
        .to_string();

    match &seen.borrow()[0] {
        Request::Apply { app, route, .. } => {
            assert_eq!(app, "myapp");
            assert_eq!(route.domains, ["myapp.example"]);
        }
        other => panic!("expected an apply, got {other:?}"),
    }
    assert!(
        err.contains("check") && err.contains("emerg: unknown directive"),
        "{err}"
    );
}

#[test]
fn route_the_gate_refuses_never_reaches_the_helper() {
    let (tmp, paths) = setup();
    let seen = RefCell::new(Vec::new());
    let env = env(&[
        ("RIKU_DOMAINS", "myapp.example"),
        ("RIKU_NGINX_CACHE_PATH", "/etc"),
    ]);

    let err = apply(
        mock_helper(Reply::Ok { reloaded: true }, &seen),
        &env,
        &tmp,
        &paths,
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("RIKU_NGINX_CACHE_PATH"), "{err}");
    assert!(seen.borrow().is_empty(), "the helper was asked anyway");
}

fn openssl_available() -> bool {
    Command::new("openssl")
        .arg("version")
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn nginx_applies_http_before_issuing() {
    if !openssl_available() {
        eprintln!("skipped: openssl is not installed");
        return;
    }
    let (tmp, paths) = setup();
    let seen = RefCell::new(Vec::new());
    let env = env(&[
        ("RIKU_DOMAINS", "myapp.example"),
        ("RIKU_HTTPS_ONLY", "true"),
    ]);

    apply(
        mock_helper(Reply::Ok { reloaded: true }, &seen),
        &env,
        &tmp,
        &paths,
    )
    .unwrap();

    let seen = seen.borrow();
    assert_eq!(seen.len(), 2, "an HTTP apply, then the HTTPS one: {seen:?}");
    let (first, second) = (applied(&seen[0]), applied(&seen[1]));
    assert_eq!(first.https, HttpsPolicy::AlsoPlainHttp);
    assert!(
        first.tls.is_none(),
        "the first apply serves the challenge over HTTP"
    );
    assert_eq!(second.https, HttpsPolicy::Only);
    assert!(second.tls.is_some(), "no certificate was sent");
}

#[test]
fn failed_fallback_certificate_serves_http_and_says_so() {
    let (tmp, paths) = setup();
    let seen = RefCell::new(Vec::new());
    let env = env(&[("RIKU_DOMAINS", "myapp.203.0.113.10.example")]);
    let routing = AppRouting {
        domains: vec!["myapp.203.0.113.10.example".to_string()],
        fallback: true,
        legacy: false,
        ..own_domains()
    };

    apply_routed(
        mock_helper(Reply::Ok { reloaded: true }, &seen),
        &env,
        &routing,
        &tmp,
        &paths,
    )
    .unwrap();

    let seen = seen.borrow();
    assert_eq!(
        seen.len(),
        1,
        "no second apply without a certificate: {seen:?}"
    );
    let route = applied(&seen[0]);
    assert_eq!(route.https, HttpsPolicy::AlsoPlainHttp);
    assert!(route.tls.is_none());
}

#[test]
fn fallback_with_a_pair_carries_the_fullchain() {
    let (tmp, paths) = setup();
    let seen = RefCell::new(Vec::new());

    fs::write(paths.nginx_root.join("myapp.key"), "KEY PEM").unwrap();
    fs::write(paths.nginx_root.join("myapp.crt"), "leaf only").unwrap();
    fs::write(paths.nginx_root.join("myapp.fullchain.crt"), "CHAIN PEM").unwrap();
    crate::router::routing_state::with_routing_lock(&paths, |state| {
        state.set_entry(
            "myapp",
            crate::router::routing_state::Entry {
                mode: RoutingMode::Domain,
                port: None,
                picked: false,
                tls: Some(crate::router::routing_state::TlsState::Issued),
                cert_for: Some("myapp.203.0.113.10.example".to_string()),
            },
        );
        Ok(())
    })
    .unwrap();
    let env = env(&[("RIKU_DOMAINS", "myapp.203.0.113.10.example")]);
    let routing = AppRouting {
        domains: vec!["myapp.203.0.113.10.example".to_string()],
        fallback: true,
        legacy: false,
        ..own_domains()
    };

    apply_routed(
        mock_helper(Reply::Ok { reloaded: true }, &seen),
        &env,
        &routing,
        &tmp,
        &paths,
    )
    .unwrap();

    let seen = seen.borrow();
    let route = applied(&seen[seen.len() - 1]);
    assert_eq!(route.https, HttpsPolicy::AlsoPlainHttp);
    assert_eq!(
        route.tls.as_ref().map(|tls| tls.cert_pem.as_str()),
        Some("CHAIN PEM")
    );
}

#[test]
fn removal_deletes_local_files_only_after_the_helper_agrees() {
    let (_tmp, paths) = setup();
    let key = paths.nginx_root.join("myapp.key");
    fs::write(&key, "pem").unwrap();
    let seen = RefCell::new(Vec::new());
    let refused = Reply::Failed {
        stage: Stage::Reload,
        error: "reload refused".to_string(),
    };

    let err = remove_nginx_site(mock_helper(refused, &seen), "myapp", &paths).unwrap_err();
    assert!(err.to_string().contains("reload refused"), "{err}");
    assert!(key.exists(), "files went before the helper agreed");

    let reloaded = remove_nginx_site(
        mock_helper(Reply::Ok { reloaded: false }, &seen),
        "myapp",
        &paths,
    )
    .unwrap();
    assert!(!reloaded);
    assert!(!key.exists());
}

#[test]
fn check_asks_the_helper_for_its_status() {
    let (_tmp, paths) = setup();
    let seen = RefCell::new(Vec::new());
    let status = Reply::Status {
        version: "0.1.2".to_string(),
    };

    check_nginx_site(mock_helper(status, &seen), "myapp", &env(&[]), &paths).unwrap();

    assert_eq!(seen.borrow().as_slice(), [Request::Status {}]);
}
