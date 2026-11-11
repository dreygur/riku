use std::net::Ipv4Addr;

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
fn catch_all_name_is_not_a_url() {
    let lines = live_lines("myapp", &routing(&["_", "a.example"], false), None, false);

    assert_eq!(texts(lines), ["Live at http://a.example"]);
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
        lines.iter().any(|l| l.contains("riku.toml")
            && l.contains("rk settings set my_app routing.domains=<domain>")),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("RIKU_DOMAINS")),
        "config set refuses RIKU_* keys: {lines:?}"
    );
}

#[test]
fn port_only_app_has_no_domain_lines() {
    let routed = AppRouting {
        port: Some(8100),
        mode: RoutingMode::Port,
        ..routing(&[], false)
    };

    assert!(live_lines("myapp", &routed, None, false).is_empty());
}

#[test]
fn port_lines_name_the_url_and_the_firewall() {
    let lines = texts(port_lines(8100, Some(Ipv4Addr::new(203, 0, 113, 10))));

    assert_eq!(lines[0], "Live at http://203.0.113.10:8100");
    assert!(
        lines
            .iter()
            .any(|l| l.contains("firewall") && l.contains("8100")),
        "{lines:?}"
    );
    assert!(lines.iter().any(|l| l.contains("HTTP only")), "{lines:?}");
}

#[test]
fn port_lines_without_server_ip_say_where_to_look() {
    let lines = texts(port_lines(8100, None));

    assert_eq!(lines[0], "Live at http://<this server's address>:8100");
}

#[test]
fn path_mode_lists_the_path_and_warns_about_the_base_path() {
    let routed = AppRouting {
        mode: RoutingMode::Path,
        ..routing(&[], false)
    };

    let mut lines = texts(live_lines("shop", &routed, None, false));
    lines.extend(texts(path_lines(
        "shop",
        8080,
        Some(Ipv4Addr::new(203, 0, 113, 10)),
    )));

    assert_eq!(lines[0], "Live at http://203.0.113.10:8080/shop/");
    assert!(
        lines.iter().any(|l| l.contains("base path /shop")),
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
