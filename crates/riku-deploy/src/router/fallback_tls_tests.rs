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
