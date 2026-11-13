use std::net::Ipv4Addr;

use super::*;
use crate::config::router::Router;
use crate::router::app_routing::{AppRouting, RoutingMode};

fn routing(domains: &[&str], fallback: bool) -> AppRouting {
    AppRouting {
        domains: domains.iter().map(|d| d.to_string()).collect(),
        fallback,
        port: None,
        mode: RoutingMode::Domain,
        fallback_skipped: None,
        fallback_private_ip: None,
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
        DomainTls::Served,
    );

    assert_eq!(
        texts(lines),
        ["Live at https://a.example", "Live at https://b.example"]
    );
}

#[test]
fn catch_all_name_is_not_a_url() {
    let lines = live_lines(
        "myapp",
        &routing(&["_", "a.example"], false),
        None,
        DomainTls::PlainHttp,
    );

    assert_eq!(texts(lines), ["Live at http://a.example"]);
}

#[test]
fn issued_fallback_lists_https() {
    let lines = live_lines(
        "myapp",
        &routing(&["myapp.203.0.113.10.example"], true),
        Some(TlsState::Issued),
        DomainTls::PlainHttp,
    );

    assert_eq!(texts(lines), ["Live at https://myapp.203.0.113.10.example"]);
}

#[test]
fn failed_fallback_says_http_only() {
    let lines = texts(live_lines(
        "myapp",
        &routing(&["myapp.203.0.113.10.example"], true),
        Some(TlsState::FailedAt(1)),
        DomainTls::PlainHttp,
    ));

    assert_eq!(lines[0], "Live at http://myapp.203.0.113.10.example");
    assert!(lines[1].contains("HTTPS"), "{lines:?}");
}

#[test]
fn fallback_on_a_private_address_says_http_only_and_why() {
    let routed = AppRouting {
        fallback_private_ip: Some(Ipv4Addr::new(10, 0, 0, 5)),
        ..routing(&["myapp.10.0.0.5.example"], true)
    };

    let lines = texts(live_lines("myapp", &routed, None, DomainTls::PlainHttp));

    assert_eq!(
        lines,
        [
            "Live at http://myapp.10.0.0.5.example",
            "HTTP only: 10.0.0.5 is a private address, so no certificate can be issued",
        ]
    );
}

#[test]
fn caddy_fallback_on_a_public_address_lists_https_while_its_certificate_is_issued() {
    let lines = texts(live_lines(
        "myapp",
        &routing(&["myapp.203.0.113.10.example"], true),
        Some(TlsState::ByCaddy),
        DomainTls::CaddyAutomatic,
    ));

    assert_eq!(
        lines,
        [
            "Live at https://myapp.203.0.113.10.example",
            "Caddy is issuing the certificate; http://myapp.203.0.113.10.example works meanwhile",
        ]
    );
}

#[test]
fn caddy_lists_https_for_own_domains_it_can_get_a_public_certificate_for() {
    let lines = texts(live_lines(
        "myapp",
        &routing(&["a.example.com", "203.0.113.5", "*.example.com"], false),
        None,
        DomainTls::CaddyAutomatic,
    ));

    assert_eq!(
        lines,
        [
            "Live at https://a.example.com",
            "Live at http://203.0.113.5",
            "Serves *.example.com: a wildcard has no single URL",
        ],
        "Caddy gives a bare IP its internal CA"
    );
}

#[test]
fn https_on_own_domains_follows_the_router_and_the_app_certificate() {
    let covered = || Some(vec!["a.example".to_string()]);
    let own = || DomainTls::OwnCert {
        covered: vec!["a.example".to_string()],
    };
    let cases = [
        (Router::Caddy, false, None, DomainTls::CaddyAutomatic),
        (Router::Caddy, false, covered(), own()),
        (Router::Nginx, false, None, DomainTls::PlainHttp),
        (Router::Nginx, false, covered(), own()),
        (Router::Nginx, true, None, DomainTls::Served),
        (Router::Nginx, true, covered(), DomainTls::Served),
        (
            Router::Plugin("x".into()),
            false,
            covered(),
            DomainTls::PlainHttp,
        ),
        (Router::Plugin("x".into()), true, None, DomainTls::Served),
    ];

    for (case, (router, https_only, covered_names, wanted)) in cases.into_iter().enumerate() {
        assert_eq!(
            domain_tls(&router, https_only, covered_names),
            wanted,
            "case {case}"
        );
    }
}

#[test]
fn own_certificate_lists_https_only_for_the_names_it_covers() {
    let lines = texts(live_lines(
        "myapp",
        &routing(&["a.example", "b.example"], false),
        None,
        DomainTls::OwnCert {
            covered: vec!["a.example".to_string()],
        },
    ));

    assert_eq!(
        lines,
        ["Live at https://a.example", "Live at http://b.example"],
        "a name outside the certificate gets a browser warning on HTTPS"
    );
}

#[test]
fn wildcard_domain_is_listed_without_a_url() {
    let lines = live_lines(
        "myapp",
        &routing(&["*.example.com"], false),
        None,
        DomainTls::Served,
    );

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].url, None, "nobody can open *.example.com");
    assert_eq!(
        lines[0].text,
        "Serves *.example.com: a wildcard has no single URL"
    );
}

#[test]
fn no_domain_no_fallback_says_how_to_add_one() {
    let routed = AppRouting {
        fallback_skipped: Some("the app name 'my_app' does not make a valid hostname".to_string()),
        ..routing(&[], false)
    };

    let lines = texts(live_lines("my_app", &routed, None, DomainTls::PlainHttp));

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

    assert!(live_lines("myapp", &routed, None, DomainTls::PlainHttp).is_empty());
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
fn a_private_address_is_labelled_lan() {
    let lan = Some(Ipv4Addr::new(10, 0, 0, 5));

    assert_eq!(
        texts(port_lines(8100, lan))[0],
        "Live at http://10.0.0.5:8100 (LAN)"
    );
    assert_eq!(
        texts(path_lines("shop", 8080, lan))[0],
        "Live at http://10.0.0.5:8080/shop/ (LAN)"
    );
}

/// The dashboard takes the URL and its label from the same lines; an
/// unknown server address is not a URL anyone can open.
#[test]
fn live_lines_carry_the_url_and_its_label() {
    let urls = |lines: Vec<Line>| -> Vec<Option<LiveUrl>> {
        lines
            .into_iter()
            .filter(|l| l.colour == "green")
            .map(|l| l.url)
            .collect()
    };
    let live = |url: &str, label| {
        Some(LiveUrl {
            url: url.to_string(),
            label,
        })
    };

    assert_eq!(
        urls(live_lines(
            "shop",
            &routing(&["a.example"], false),
            None,
            DomainTls::Served
        )),
        [live("https://a.example", UrlLabel::Domain)]
    );
    assert_eq!(
        urls(port_lines(8100, Some(Ipv4Addr::new(203, 0, 113, 10)))),
        [live("http://203.0.113.10:8100", UrlLabel::Public)]
    );
    assert_eq!(
        urls(path_lines("shop", 8080, Some(Ipv4Addr::new(10, 0, 0, 2)))),
        [live("http://10.0.0.2:8080/shop/", UrlLabel::Lan)]
    );
    assert_eq!(urls(port_lines(8100, None)), [None]);
}

#[test]
fn port_lines_without_server_ip_say_where_to_look() {
    let lines = texts(port_lines(8100, None));

    assert_eq!(lines[0], "Live at http://<this server's address>:8100");
}

#[test]
fn url_host_falls_back_to_the_detected_address_when_none_is_configured() {
    let public = Ipv4Addr::new(203, 0, 113, 10);
    let private = Ipv4Addr::new(10, 0, 0, 5);

    assert_eq!(
        url_host(Some(public), Some(private.into())),
        Some(public),
        "server_ip from settings wins"
    );
    assert_eq!(
        url_host(None, Some(private.into())),
        Some(private),
        "a server with no public address still shows the one it has"
    );
    assert_eq!(
        url_host(None, None),
        None,
        "keep the placeholder when nothing is known"
    );
}

#[test]
fn path_mode_lists_the_path_and_warns_about_the_base_path() {
    let routed = AppRouting {
        mode: RoutingMode::Path,
        ..routing(&[], false)
    };

    let mut lines = texts(live_lines("shop", &routed, None, DomainTls::PlainHttp));
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
