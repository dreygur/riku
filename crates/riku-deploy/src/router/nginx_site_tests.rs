use std::cell::RefCell;
use std::fs;
use std::process::{Command, Stdio};

use riku_route::Stage;

use super::*;

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
    apply_nginx_site(
        send,
        "myapp",
        tmp.path(),
        env,
        &[Backend::loopback(5000)],
        paths,
    )
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
        Request::Apply { app, route } => {
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
fn first_https_only_deploy_sends_a_certificate() {
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

    let sent = seen.borrow()[0].clone();
    match sent {
        Request::Apply { route, .. } => assert!(route.tls.is_some(), "no certificate was sent"),
        other => panic!("expected an apply, got {other:?}"),
    }
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
    let (tmp, paths) = setup();
    let seen = RefCell::new(Vec::new());
    let status = Reply::Status {
        version: "0.1.2".to_string(),
    };

    check_nginx_site(
        mock_helper(status, &seen),
        "myapp",
        tmp.path(),
        &env(&[]),
        &paths,
    )
    .unwrap();

    assert_eq!(seen.borrow().as_slice(), [Request::Status {}]);
}
