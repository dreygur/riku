use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use riku_util::route_plan::HttpsPolicy;

use super::*;
use crate::check_text::{header_text, relative_file};
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
        cache: None,
        cloudflare_acl: false,
        allow_git_folders: false,
        catch_all: None,
        csp: None,
        snippet: None,
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

fn cache() -> Cache {
    Cache {
        prefixes: vec!["/api/".to_string()],
        time_s: 60,
        size_gb: 1,
        expiry_s: 60,
        redirects_s: 60,
        any_s: 60,
        control_s: 60,
    }
}

#[test]
fn a_plain_route_passes() {
    let mut full = route();
    full.tls = tls(CERT_A, KEY_A);
    full.static_paths = vec!["/static/".to_string()];
    full.snippet = Some("add_header X-Frame-Options DENY;".to_string());
    full.csp = Some("default-src 'self'; img-src *".to_string());
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
fn filtered_snippet_outside_allowlist_is_refused() {
    let mut risky = route();
    risky.snippet = Some("access_log /etc/cron.d/x;".to_string());
    assert!(refusal(&risky).contains("access_log"));

    risky.snippet = Some("add_header X-A 1;\nproxy_pass http://127.0.0.1:1;".to_string());
    assert!(refusal(&risky).contains("proxy_pass"));
}

#[test]
fn quoted_and_file_text_refusals() {
    for bad in ["a\"b", "a$b", "a\\b", "a\nb"] {
        assert!(header_text("csp", bad).is_err(), "{bad:?}");
    }
    header_text("csp", "default-src 'self'; img-src *").unwrap();
    for bad in ["../x", "a/../x", "a\nb", "a\"b", "/etc/passwd", ""] {
        assert!(relative_file("catch_all", bad).is_err(), "{bad:?}");
    }
    relative_file("catch_all", "index.html").unwrap();
}

#[test]
fn cache_limits_are_enforced() {
    let mut limited = route();
    limited.cache = Some(Cache {
        size_gb: 11,
        ..cache()
    });
    assert!(refusal(&limited).contains("size_gb"));

    limited.cache = Some(Cache {
        time_s: u32::MAX,
        ..cache()
    });
    assert!(refusal(&limited).contains("time_s"));

    limited.cache = Some(Cache {
        prefixes: Vec::new(),
        ..cache()
    });
    assert!(refusal(&limited).contains("prefix"));

    limited.cache = Some(Cache {
        prefixes: vec!["/a;".to_string()],
        ..cache()
    });
    assert!(refusal(&limited).contains("cache prefix"));
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
    with(|r| r.snippet = Some("add_header X-A 1;".to_string()));
    with(|r| r.tls = tls(CERT_A, KEY_A));
    with(|r| r.csp = Some("default-src 'self'".to_string()));
    with(|r| r.cache = Some(cache()));
    with(|r| r.catch_all = Some("index.html".to_string()));
    with(|r| r.cloudflare_acl = true);
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
