use std::path::PathBuf;

use super::*;

const SOCKET: &str = "/run/riku-sockets/myapp.sock";

struct Fixture {
    app: &'static str,
    shape: SiteShape,
    https_only: bool,
    also_tls: bool,
    open_port: Option<u16>,
    domains: Vec<String>,
    ipv6: bool,
    backends: Vec<Backend>,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            app: "myapp",
            shape: SiteShape::Plain,
            https_only: false,
            also_tls: false,
            open_port: None,
            domains: vec!["myapp.example.com".to_string()],
            ipv6: true,
            backends: vec![Backend::Unix(SOCKET.into())],
        }
    }

    fn render(&self) -> Result<String> {
        let files = SiteFiles {
            certificate: PathBuf::from("/etc/riku/certs/myapp/fullchain.pem"),
            certificate_key: PathBuf::from("/etc/riku/certs/myapp/key.pem"),
            acme_root: PathBuf::from("/var/lib/riku/acme"),
            static_root: PathBuf::from("/var/lib/riku/static"),
        };

        render_route_site(&RouteSite {
            app: self.app,
            domains: &self.domains,
            shape: self.shape,
            https_only: self.https_only,
            also_tls: self.also_tls,
            open_port: self.open_port,
            port: 80,
            backends: &self.backends,
            static_locations: &[],
            allow_git_folders: false,
            ipv6: self.ipv6,
            files: &files,
        })
    }
}

#[test]
fn upstream_names_follow_the_app_name_case_free() {
    let upper = Fixture {
        app: "MyApp",
        shape: SiteShape::Portmap,
        backends: vec![Backend::loopback(5001), Backend::loopback(5002)],
        ..Fixture::new()
    };

    let config = upper.render().unwrap();

    assert!(
        config.contains(
            "upstream riku_4d79417070 {\n  server 127.0.0.1:5001;\n  server 127.0.0.1:5002;\n}"
        ),
        "{config}"
    );
    assert!(
        config.contains("proxy_pass http://riku_4d79417070;"),
        "{config}"
    );
}

#[test]
fn socket_upstreams_always_carry_the_unix_prefix() {
    let site = Fixture {
        shape: SiteShape::Wsgi,
        backends: vec![
            Backend::Unix(SOCKET.into()),
            Backend::Unix("/run/riku-sockets/myapp@2.sock".into()),
        ],
        ..Fixture::new()
    };

    let config = site.render().unwrap();

    assert!(
        config.contains(&format!(
            "upstream riku_6d79617070 {{\n  server unix:{SOCKET};\n  server unix:/run/riku-sockets/myapp@2.sock;\n}}"
        )),
        "{config}"
    );
    assert!(config.contains("uwsgi_pass riku_6d79617070;"), "{config}");
}

#[test]
fn issued_fallback_serves_both_without_redirect() {
    for shape in [SiteShape::Portmap, SiteShape::Plain] {
        let site = Fixture {
            shape,
            also_tls: true,
            backends: vec![Backend::loopback(5001)],
            ..Fixture::new()
        };

        let config = site.render().unwrap();

        assert!(
            config.contains("listen              0.0.0.0:443 ssl;"),
            "{config}"
        );
        assert!(
            config.contains("listen              0.0.0.0:80;"),
            "{config}"
        );
        assert!(
            config.contains("ssl_certificate     /etc/riku/certs/myapp/fullchain.pem;"),
            "{config}"
        );
        assert!(!config.contains("return 301"), "no redirect: {config}");
    }
}

#[test]
fn nginx_renders_catch_all_server_on_open_port() {
    let site = Fixture {
        shape: SiteShape::Portmap,
        domains: Vec::new(),
        open_port: Some(8100),
        ipv6: false,
        backends: vec![Backend::loopback(5001)],
        ..Fixture::new()
    };

    let config = site.render().unwrap();

    assert!(
        config.contains("listen              0.0.0.0:8100;"),
        "{config}"
    );
    assert!(
        !config.contains("0.0.0.0:80;"),
        "a port only app must not claim 80: {config}"
    );
    assert!(config.contains("server_name         _;"), "{config}");
}

#[test]
fn domain_and_port_render_both() {
    for (shape, https_only) in [
        (SiteShape::Portmap, false),
        (SiteShape::Plain, false),
        (SiteShape::Plain, true),
    ] {
        let site = Fixture {
            shape,
            https_only,
            open_port: Some(8100),
            backends: vec![Backend::loopback(5001)],
            ..Fixture::new()
        };

        let config = site.render().unwrap();

        assert!(
            config.contains("listen              0.0.0.0:8100;"),
            "{config}"
        );
        assert!(
            config.contains("server_name         myapp.example.com;"),
            "{config}"
        );
    }
}

#[test]
fn plain_site_without_tls_has_no_443() {
    let config = Fixture::new().render().unwrap();

    assert!(!config.contains("443"), "{config}");
}

#[test]
fn https_only_answers_acme_on_plain_http_and_once_on_tls() {
    let site = Fixture {
        https_only: true,
        ..Fixture::new()
    };

    let config = site.render().unwrap();
    let (plain, tls) = config.split_once("443 ssl").expect("not an https config");

    assert!(
        plain.contains("location / {\n    return 301"),
        "redirect must sit in location /, or the challenge is never answered:\n{plain}"
    );
    assert_eq!(
        tls.matches("location ^~ /.well-known/acme-challenge")
            .count(),
        1,
        "nginx refuses a duplicate location:\n{tls}"
    );
}

#[test]
fn ipv6_listen_lines_follow_the_host() {
    for (shape, https_only) in [
        (SiteShape::Plain, false),
        (SiteShape::Plain, true),
        (SiteShape::Portmap, false),
        (SiteShape::Wsgi, false),
        (SiteShape::Static, false),
    ] {
        for ipv6 in [false, true] {
            let site = Fixture {
                shape,
                https_only,
                ipv6,
                ..Fixture::new()
            };

            let config = site.render().unwrap();

            assert_eq!(
                config.contains("[::]"),
                ipv6,
                "{shape:?} {https_only}:\n{config}"
            );
        }
    }
}

#[test]
fn plain_and_https_proxies_pass_websocket_upgrades() {
    for https_only in [false, true] {
        let site = Fixture {
            https_only,
            backends: vec![Backend::loopback(5001)],
            ..Fixture::new()
        };

        let config = site.render().unwrap();

        assert!(
            config.contains("proxy_set_header Upgrade $http_upgrade;")
                && config.contains("proxy_set_header Connection \"upgrade\";"),
            "https_only {https_only}:\n{config}"
        );
    }
}

#[test]
fn backend_with_directive_characters_or_whitespace_is_refused() {
    for hostile in ["/s.sock; include /etc/passwd", "/x.sock weight=0"] {
        let site = Fixture {
            backends: vec![Backend::Unix(hostile.into())],
            ..Fixture::new()
        };

        assert!(site.render().is_err(), "{hostile:?} was rendered");
    }
}
