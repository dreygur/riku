use riku_util::backend::Backend;
use riku_util::domain::parse_domains;

use super::*;

fn plan(https: HttpsPolicy, backends: Vec<Backend>) -> RoutePlan {
    RoutePlan {
        domains: parse_domains("shop.example.com www.shop.example.com").unwrap(),
        https,
        backends,
        open_port: None,
    }
}

fn request(https: HttpsPolicy) -> RoutePlan {
    plan(https, vec![Backend::loopback(5000)])
}

#[test]
fn own_certificate_site_names_the_pair_before_the_proxy() {
    let tls = TlsFiles {
        certificate: "/etc/riku/caddy/certs/shop/fullchain.pem".into(),
        key: "/etc/riku/caddy/certs/shop/key.pem".into(),
    };

    let site = render_site_with_tls("shop", &request(HttpsPolicy::Only), Some(&tls)).unwrap();

    assert_eq!(
        site,
        "# riku app: shop\n\
        shop.example.com, www.shop.example.com {\n\
        \ttls \"/etc/riku/caddy/certs/shop/fullchain.pem\" \"/etc/riku/caddy/certs/shop/key.pem\"\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n"
    );
}

/// Caddy refuses a `tls` line in a block that also listens on plain HTTP
/// ("server listening on [:80] is HTTP, but attempts to configure TLS
/// connection policies"), checked against caddy 2 on 2026-11-11.
#[test]
fn own_certificate_keeps_plain_http_addresses_in_their_own_block() {
    let tls = TlsFiles {
        certificate: "/c/fullchain.pem".into(),
        key: "/c/key.pem".into(),
    };
    let plan = RoutePlan {
        open_port: Some(8100),
        ..request(HttpsPolicy::AlsoPlainHttp)
    };

    let site = render_site_with_tls("shop", &plan, Some(&tls)).unwrap();

    assert_eq!(
        site,
        "# riku app: shop\n\
        shop.example.com, www.shop.example.com {\n\
        \ttls \"/c/fullchain.pem\" \"/c/key.pem\"\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n\
        http://shop.example.com, http://www.shop.example.com, http://:8100 {\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n"
    );
}

#[test]
fn a_certificate_path_that_could_break_out_of_its_quotes_is_refused() {
    let tls = TlsFiles {
        certificate: "/etc/riku/caddy/certs/shop/a\" {\nrespond".into(),
        key: "/etc/riku/caddy/certs/shop/key.pem".into(),
    };

    let e = render_site_with_tls("shop", &request(HttpsPolicy::Only), Some(&tls)).unwrap_err();

    assert!(e.to_string().contains("certificate path"), "{e}");
}

#[test]
fn caddy_takes_open_port() {
    let port_only = RoutePlan {
        domains: Vec::new(),
        open_port: Some(8100),
        ..request(HttpsPolicy::AlsoPlainHttp)
    };
    let both = RoutePlan {
        open_port: Some(8100),
        ..request(HttpsPolicy::Only)
    };

    assert!(
        render_site("shop", &port_only)
            .unwrap()
            .contains("\nhttp://:8100 {\n"),
        "{}",
        render_site("shop", &port_only).unwrap()
    );
    assert!(
        render_site("shop", &both)
            .unwrap()
            .contains("shop.example.com, www.shop.example.com, http://:8100 {"),
        "{}",
        render_site("shop", &both).unwrap()
    );
}

#[test]
fn https_only_site() {
    let expected = "# riku app: shop\n\
        shop.example.com, www.shop.example.com {\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n";

    assert_eq!(
        render_site("shop", &request(HttpsPolicy::Only)).unwrap(),
        expected
    );
}

#[test]
fn plain_site_adds_http() {
    let expected = "# riku app: shop\n\
        shop.example.com, www.shop.example.com, http://shop.example.com, http://www.shop.example.com {\n\
        \treverse_proxy 127.0.0.1:5000\n\
        }\n";

    assert_eq!(
        render_site("shop", &request(HttpsPolicy::AlsoPlainHttp)).unwrap(),
        expected
    );
}

#[test]
fn every_backend_is_an_upstream() {
    let plan = plan(
        HttpsPolicy::Only,
        vec![Backend::loopback(5001), Backend::loopback(5002)],
    );

    assert!(render_site("shop", &plan)
        .unwrap()
        .contains("\treverse_proxy 127.0.0.1:5001 127.0.0.1:5002\n"));
}

#[test]
fn socket_backend_is_refused() {
    let plan = plan(HttpsPolicy::Only, vec![Backend::Unix("/x.sock".into())]);

    let err = render_site("shop", &plan).unwrap_err();

    assert!(err.to_string().contains("TCP only"), "{err}");
}

#[test]
fn empty_backend_list_is_refused() {
    let err = render_site("shop", &plan(HttpsPolicy::Only, vec![])).unwrap_err();

    assert!(err.to_string().contains("no backend"), "{err}");
}
