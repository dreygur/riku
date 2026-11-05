use std::net::Ipv4Addr;
use std::path::PathBuf;

use super::*;

const SOCKET: &str = "/run/riku-sockets/myapp.sock";

struct Fixture {
    app: &'static str,
    shape: SiteShape,
    https_only: bool,
    backends: Vec<Backend>,
    csp: Option<&'static str>,
    snippet: &'static str,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture {
            app: "myapp",
            shape: SiteShape::Plain,
            https_only: false,
            backends: vec![Backend::Unix(SOCKET.into())],
            csp: None,
            snippet: "",
        }
    }

    fn render(&self) -> Result<String> {
        let files = SiteFiles {
            certificate: PathBuf::from("/etc/riku/certs/myapp/fullchain.pem"),
            certificate_key: PathBuf::from("/etc/riku/certs/myapp/key.pem"),
            cloudflare_conf: PathBuf::from("/etc/riku/cloudflare.conf"),
            cache_dir: PathBuf::from("/var/cache/riku/nginx/myapp"),
            acme_root: PathBuf::from("/var/lib/riku/acme"),
            static_root: PathBuf::from("/var/lib/riku/static"),
        };
        let domains = vec!["myapp.example.com".to_string()];
        render_route_site(&RouteSite {
            app: self.app,
            domains: &domains,
            shape: self.shape,
            https_only: self.https_only,
            ipv4: Ipv4Addr::UNSPECIFIED,
            ipv6: None,
            port: 80,
            backends: &self.backends,
            static_locations: &[],
            cache: None,
            cloudflare_acl: false,
            allow_git_folders: false,
            catch_all: None,
            csp: self.csp,
            snippet: self.snippet,
            files: &files,
        })
    }
}

#[test]
fn csp_that_closes_the_string_is_refused() {
    let site = Fixture {
        csp: Some("default-src 'self'\"; access_log /etc/cron.d/x; \""),
        ..Fixture::new()
    };

    let err = site.render().unwrap_err();

    assert!(format!("{err:#}").contains("csp"), "{err:#}");
}

#[test]
fn csp_with_semicolons_and_quotes_renders() {
    let site = Fixture {
        csp: Some("default-src 'self'; img-src *"),
        ..Fixture::new()
    };

    let config = site.render().unwrap();

    assert!(
        config.contains("Content-Security-Policy \"default-src 'self'; img-src *\""),
        "{config}"
    );
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
fn a_snippet_directive_replaces_the_templates_own_line() {
    for (shape, https_only) in [
        (SiteShape::Portmap, false),
        (SiteShape::Wsgi, false),
        (SiteShape::Plain, true),
    ] {
        let site = Fixture {
            shape,
            https_only,
            snippet: "client_max_body_size 50m;\ngzip off;\n",
            ..Fixture::new()
        };

        let config = site.render().unwrap();

        assert_eq!(
            config.matches("client_max_body_size").count(),
            1,
            "{shape:?}:\n{config}"
        );
        assert!(
            config.contains("gzip off;") && !config.contains("gzip on;"),
            "{shape:?}:\n{config}"
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
