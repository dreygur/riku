use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use riku_util::route_plan::HttpsPolicy;

use super::*;
use crate::schema::{Listen, Tls};
use crate::test_pems::{CERT_A, KEY_A, KEY_B};

fn app() -> AppName {
    AppName::try_from("myapp").unwrap()
}

fn route() -> Route {
    Route {
        domains: vec!["myapp.example".to_string()],
        https: HttpsPolicy::AlsoPlainHttp,
        backends: vec![RouteBackend::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 5000,
        }],
        listen: Listen::default(),
        tls: None,
        static_paths: Vec::new(),
        kind: SiteKind::Plain,
        allow_git_folders: false,
        open_port: None,
    }
}

fn refusal(route: &Route) -> String {
    match check_nginx_route(&app(), route) {
        Ok(_) => panic!("route was accepted: {route:?}"),
        Err(e) => e.to_string(),
    }
}

fn tls(cert: &str, key: &str) -> Option<Tls> {
    Some(Tls {
        cert_pem: cert.to_string(),
        key_pem: key.to_string(),
    })
}

#[test]
fn a_plain_route_passes() {
    let mut full = route();
    full.tls = tls(CERT_A, KEY_A);
    full.static_paths = vec!["/static/".to_string()];
    full.domains.push("_".to_string());
    full.backends.push(RouteBackend::UnixDefault);

    check_nginx_route(&app(), &full).unwrap();
}

#[test]
fn directive_break_in_domain_is_refused() {
    let mut broken = route();
    broken.domains = vec!["myapp.example;".to_string()];
    assert!(refusal(&broken).contains("myapp.example;"));

    broken.domains = vec!["Myapp.example".to_string()];
    assert!(refusal(&broken).contains("lowercase"));
}

#[test]
fn directive_break_in_a_location_is_refused() {
    let mut broken = route();
    broken.static_paths = vec!["/a { access_log /etc/cron.d/x; }".to_string()];

    assert!(refusal(&broken).contains("static_paths"));
}

#[test]
fn public_tcp_backend_is_refused() {
    for host in [
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        "::ffff:127.0.0.1".parse().unwrap(),
    ] {
        let mut public = route();
        public.backends = vec![RouteBackend::Tcp { host, port: 5000 }];

        assert!(
            refusal(&public).contains("not a loopback address"),
            "{host}"
        );
    }
}

#[test]
fn copy_number_zero_is_refused() {
    let mut zero = route();
    zero.backends = vec![RouteBackend::UnixCopy(0)];

    assert!(refusal(&zero).contains("copy number"));
}

#[test]
fn privileged_listen_ports_are_refused() {
    let mut low = route();
    for port in [22, 443, 1023] {
        low.listen.port = port;
        assert!(refusal(&low).contains("privileged"), "{port}");
    }
    for port in [80, 1024, 8080] {
        low.listen.port = port;
        check_nginx_route(&app(), &low).unwrap();
    }
}

#[test]
fn mismatched_tls_pair_is_refused() {
    let mut mismatched = route();
    mismatched.tls = tls(CERT_A, KEY_B);
    assert!(refusal(&mismatched).contains("does not match"));

    let hidden_key = format!("{CERT_A}{KEY_A}");
    mismatched.tls = tls(&hidden_key, KEY_A);
    assert!(refusal(&mismatched).contains("not a certificate"));

    let two_keys = format!("{KEY_A}{KEY_B}");
    mismatched.tls = tls(CERT_A, &two_keys);
    assert!(refusal(&mismatched).contains("exactly one private key"));
}

#[test]
fn tls_text_is_rewritten_from_the_parsed_der() {
    let cert = format!(
        "junk; access_log /etc/cron.d/x;\n-----BEGIN TRUSTED CERTIFICATE-----\nAAAA\n-----END TRUSTED CERTIFICATE-----\n{CERT_A}-----BEGIN ENCRYPTED PRIVATE KEY-----\nAAAA\n-----END ENCRYPTED PRIVATE KEY-----\ntrailing text\n"
    );
    let key = format!(
        "-----BEGIN EC PARAMETERS-----\nBggqhkjOPQMBBw==\n-----END EC PARAMETERS-----\n{KEY_A}"
    );
    let mut sent = route();
    sent.tls = tls(&cert, &key);

    let checked = check_nginx_route(&app(), &sent).unwrap();
    let out = checked.route().tls.clone().unwrap();

    assert_eq!(out.cert_pem, CERT_A);
    assert_eq!(out.key_pem, KEY_A);
}

#[test]
fn https_only_needs_a_tls_pair() {
    let mut only = route();
    only.https = HttpsPolicy::Only;
    assert!(refusal(&only).contains("no tls"));

    only.tls = tls(CERT_A, KEY_A);
    check_nginx_route(&app(), &only).unwrap();
}

#[test]
fn a_static_site_needs_no_backend() {
    let mut site = route();
    site.kind = SiteKind::Static;
    site.backends = Vec::new();
    check_nginx_route(&app(), &site).unwrap();

    site.kind = SiteKind::Plain;
    assert!(refusal(&site).contains("at least one backend"));
}

#[test]
fn caddy_refuses_what_only_nginx_does() {
    let mut refused = Vec::new();
    let mut with = |change: fn(&mut Route)| {
        let mut r = route();
        change(&mut r);
        refused.push(r);
    };
    with(|r| r.tls = tls(CERT_A, KEY_A));
    with(|r| r.listen.port = 8080);
    with(|r| r.backends = vec![RouteBackend::UnixCopy(1)]);
    with(|r| r.backends = vec![RouteBackend::UnixDefault]);
    with(|r| r.static_paths = vec!["/static/".to_string()]);
    with(|r| r.kind = SiteKind::Static);
    with(|r| r.kind = SiteKind::Wsgi);
    with(|r| r.domains = vec!["_".to_string()]);

    for r in &refused {
        let e = check_caddy_route(&app(), r).unwrap_err();
        assert!(e.to_string().contains("Caddy does not take"), "{r:?}: {e}");
    }
    check_caddy_route(&app(), &route()).unwrap();
}

fn port_only(port: u16) -> Route {
    Route {
        domains: Vec::new(),
        open_port: Some(port),
        ..route()
    }
}

#[test]
fn open_port_below_1024_or_caddy_admin_is_refused_by_gate() {
    for port in [80, 443, 22, 2019] {
        let message = refusal(&port_only(port));
        assert!(message.contains("open_port"), "{port}: {message}");
    }
}

#[test]
fn open_port_on_wsgi_or_static_site_is_refused() {
    for kind in [SiteKind::Wsgi, SiteKind::Static] {
        let route = Route {
            kind,
            ..port_only(8100)
        };
        assert!(refusal(&route).contains("open_port"), "{kind:?}");
    }
}

#[test]
fn port_only_route_passes_both_gates() {
    check_nginx_route(&app(), &port_only(8100)).unwrap();
    check_caddy_route(&app(), &port_only(8100)).unwrap();
}

#[test]
fn no_domains_and_no_open_port_is_refused() {
    let route = Route {
        domains: Vec::new(),
        ..route()
    };

    assert!(refusal(&route).contains("domain"));
}
