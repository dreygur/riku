use std::cell::Cell;

use super::*;
use crate::router::routing_state::{with_routing_lock, Entry, RoutingMode, RoutingState};

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    with_routing_lock(&paths, |state| {
        state.set_entry(
            "myapp",
            Entry {
                mode: RoutingMode::Domain,
                port: None,
                picked: false,
                tls: None,
                cert_for: None,
                domains: Vec::new(),
                fallback: false,
            },
        );
        Ok(())
    })
    .unwrap();
    (tmp, paths)
}

fn set_tls(paths: &RikuPaths, tls: TlsState) {
    with_routing_lock(paths, |state| {
        let mut entry = state.entry("myapp").cloned().unwrap();
        entry.tls = Some(tls);
        state.set_entry("myapp", entry);
        Ok(())
    })
    .unwrap();
}

fn recorded(paths: &RikuPaths) -> Option<TlsState> {
    RoutingState::load(paths)
        .unwrap()
        .entry("myapp")
        .and_then(|entry| entry.tls)
}

#[test]
fn a_pair_is_stale_when_the_name_it_covers_changes() {
    assert!(!pair_is_stale(None, None), "an own domain pair stays");
    assert!(!pair_is_stale(Some("a.example"), Some("a.example")));
    assert!(
        pair_is_stale(Some("a.example"), None),
        "fallback pair on an own domain"
    );
    assert!(
        pair_is_stale(None, Some("a.example")),
        "own domain pair on the fallback"
    );
    assert!(
        pair_is_stale(Some("a.example"), Some("b.example")),
        "server_ip changed"
    );
}

#[test]
fn drop_stale_pair_removes_the_files_and_the_record() {
    let (_tmp, paths) = paths();
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
    for suffix in ["key", "crt", "fullchain.crt"] {
        std::fs::write(paths.nginx_root.join(format!("myapp.{suffix}")), "pem").unwrap();
    }
    let outcome =
        fallback_certificate_with(|_, _, _| Ok(true), "myapp", "old.example", &paths, 1000);
    assert_eq!(outcome, TlsState::Issued);

    drop_stale_pair("myapp", None, &paths);

    for suffix in ["key", "crt", "fullchain.crt"] {
        assert!(
            !paths.nginx_root.join(format!("myapp.{suffix}")).exists(),
            "{suffix} kept"
        );
    }
    assert_eq!(recorded(&paths), None);
}

#[test]
fn drop_stale_pair_keeps_an_own_certificate() {
    let (_tmp, paths) = paths();
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
    for suffix in ["key", "crt", "fullchain.crt", "own-cert"] {
        std::fs::write(paths.nginx_root.join(format!("myapp.{suffix}")), "pem").unwrap();
    }
    fallback_certificate_with(|_, _, _| Ok(true), "myapp", "old.example", &paths, 1000);

    drop_stale_pair("myapp", None, &paths);

    for suffix in ["key", "crt", "fullchain.crt", "own-cert"] {
        assert!(
            paths.nginx_root.join(format!("myapp.{suffix}")).exists(),
            "{suffix} was deleted"
        );
    }
}

#[test]
fn issued_certificate_is_recorded() {
    let (_tmp, paths) = paths();

    let outcome =
        fallback_certificate_with(|_, _, _| Ok(true), "myapp", "myapp.example", &paths, 1000);

    assert_eq!(outcome, TlsState::Issued);
    assert_eq!(recorded(&paths), Some(TlsState::Issued));
}

#[test]
fn failed_issue_is_recorded_with_its_time() {
    let (_tmp, paths) = paths();

    let outcome =
        fallback_certificate_with(|_, _, _| Ok(false), "myapp", "myapp.example", &paths, 1000);

    assert_eq!(outcome, TlsState::FailedAt(1000));
    assert_eq!(recorded(&paths), Some(TlsState::FailedAt(1000)));
}

#[test]
fn failed_issue_is_not_retried_within_an_hour() {
    let (_tmp, paths) = paths();
    set_tls(&paths, TlsState::FailedAt(1000));
    let calls = Cell::new(0);
    let issue = |_: &str, _: &str, _: &RikuPaths| {
        calls.set(calls.get() + 1);
        Ok(true)
    };

    let soon = fallback_certificate_with(issue, "myapp", "myapp.example", &paths, 1000 + 3599);
    let later = fallback_certificate_with(issue, "myapp", "myapp.example", &paths, 1000 + 3600);

    assert_eq!(soon, TlsState::FailedAt(1000));
    assert_eq!(later, TlsState::Issued);
    assert_eq!(calls.get(), 1);
}
