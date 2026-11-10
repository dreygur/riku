use super::*;
use crate::router::app_routing::{AppRouting, RoutingMode};

fn routing(domains: &[&str], fallback: bool) -> AppRouting {
    AppRouting {
        domains: domains.iter().map(|d| d.to_string()).collect(),
        fallback,
        port: None,
        mode: RoutingMode::Domain,
        legacy: false,
        fallback_skipped: None,
    }
}

fn texts(lines: Vec<Line>) -> Vec<String> {
    lines.into_iter().map(|line| line.text).collect()
}

#[test]
fn announce_lists_each_domain() {
    let lines = live_lines(
        "myapp",
        &routing(&["a.example", "b.example"], false),
        None,
        true,
    );

    assert_eq!(
        texts(lines),
        ["Live at https://a.example", "Live at https://b.example"]
    );
}

#[test]
fn issued_fallback_lists_https() {
    let lines = live_lines(
        "myapp",
        &routing(&["myapp.203.0.113.10.example"], true),
        Some(TlsState::Issued),
        false,
    );

    assert_eq!(texts(lines), ["Live at https://myapp.203.0.113.10.example"]);
}

#[test]
fn failed_fallback_says_http_only() {
    let lines = texts(live_lines(
        "myapp",
        &routing(&["myapp.203.0.113.10.example"], true),
        Some(TlsState::FailedAt(1)),
        false,
    ));

    assert_eq!(lines[0], "Live at http://myapp.203.0.113.10.example");
    assert!(lines[1].contains("HTTPS"), "{lines:?}");
}

#[test]
fn no_domain_no_fallback_says_how_to_add_one() {
    let routed = AppRouting {
        fallback_skipped: Some("the app name 'my_app' does not make a valid hostname".to_string()),
        ..routing(&[], false)
    };

    let lines = texts(live_lines("my_app", &routed, None, false));

    assert!(
        lines.iter().any(|l| l.contains("my_app' does not make")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("domains")
            && l.contains("riku.toml")
            && l.contains("rk config set")),
        "{lines:?}"
    );
}

#[test]
fn legacy_server_points_at_doctor_fix() {
    let routed = AppRouting {
        legacy: true,
        ..routing(&[], false)
    };

    let lines = texts(live_lines("myapp", &routed, None, false));

    assert!(
        lines.iter().any(|l| l.contains("riku doctor --fix")),
        "{lines:?}"
    );
}
