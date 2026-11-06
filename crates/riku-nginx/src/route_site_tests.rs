use std::path::PathBuf;

use super::*;

const SOCKET: &str = "/run/riku-sockets/myapp.sock";

struct Fixture {
    app: &'static str,
    shape: SiteShape,
    https_only: bool,
    ipv6: bool,
    backends: Vec<Backend>,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            app: "myapp",
            shape: SiteShape::Plain,
            https_only: false,
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
        let domains = vec!["myapp.example.com".to_string()];
        render_route_site(&RouteSite {
            app: self.app,
            domains: &domains,
            shape: self.shape,
            https_only: self.https_only,
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
