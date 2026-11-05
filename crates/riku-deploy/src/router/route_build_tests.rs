use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;
use crate::router::tests::env;
use riku_route::{Cache, Listen, Tls};

const CERT: &str = "-----BEGIN CERTIFICATE-----
MIIBYzCCAQqgAwIBAgIUEgDLAO9GxEin/GHuNm5MpOnpncowCgYIKoZIzj0EAwIw
FzEVMBMGA1UEAwwMZWRnZS5leGFtcGxlMCAXDTI2MTEwNTEyMjUxN1oYDzIxMjYx
MDEyMTIyNTE3WjAXMRUwEwYDVQQDDAxlZGdlLmV4YW1wbGUwWTATBgcqhkjOPQIB
BggqhkjOPQMBBwNCAAS+7RY71t9E246At3BTwwbr51DrowJ6uHPDnOomjbnNV01b
04id+KyY90Zw18ywqMaRDMG/+gHhYALZ7hagTw3uozIwMDAdBgNVHQ4EFgQUQSGx
LwXzOj6kXaDP1/Ixu7rtIDQwDwYDVR0TAQH/BAUwAwEB/zAKBggqhkjOPQQDAgNH
ADBEAiAgdb70wvVqFKb5RNemibpEcrjI/z9B/TbP6bG+SMi+ewIgC/n99Anfg9Qh
ywPUU8ilQyU1WOhEi9yDC03jdknWmNE=
-----END CERTIFICATE-----
";

const KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgtqsfBaLAqMdhtWe1
R7eNkLHSrScB6cJOCqtBtRmr6rOhRANCAAS+7RY71t9E246At3BTwwbr51DrowJ6
uHPDnOomjbnNV01b04id+KyY90Zw18ywqMaRDMG/+gHhYALZ7hagTw3u
-----END PRIVATE KEY-----
";

fn app() -> AppName {
    AppName::try_from("myapp").unwrap()
}

fn paths_in(tmp: &TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.nginx_root).unwrap();
    paths
}

fn build_for(
    tmp: &TempDir,
    router: &Router,
    pairs: &[(&str, &str)],
    backends: &[Backend],
) -> Result<Option<Route>> {
    let paths = paths_in(tmp);
    build_route(&app(), tmp.path(), &env(pairs), backends, &paths, router)
}

fn build(tmp: &TempDir, pairs: &[(&str, &str)], backends: &[Backend]) -> Result<Route> {
    build_for(tmp, &Router::Nginx, pairs, backends).map(|route| route.expect("nginx always routes"))
}

#[test]
fn env_maps_to_route() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    fs::write(
        tmp.path().join("extra.inc"),
        "add_header X-Frame-Options DENY;\n",
    )
    .unwrap();
    let backends = [
        Backend::loopback(5001),
        Backend::Unix(paths.socket_root.join("myapp@2.sock")),
    ];

    // RIKU_NGINX_STATIC is set too: select_template tries the port mapping
    // first, so the site is a port mapped one.
    let route = build(
        &tmp,
        &[
            ("RIKU_DOMAINS", "Myapp.example www.myapp.example"),
            ("RIKU_NGINX_PORTMAP", "80:5001"),
            ("RIKU_NGINX_EXTERNAL_PORT", "8080"),
            ("RIKU_NGINX_IPV4_ADDRESS", "192.0.2.10"),
            ("RIKU_DISABLE_IPV6", "true"),
            ("RIKU_NGINX_STATIC", "public"),
            ("RIKU_NGINX_STATIC_PATHS", "/assets:dist, /media:uploads"),
            ("RIKU_NGINX_CACHE_PREFIXES", "/api/, /feed"),
            ("RIKU_NGINX_CACHE_SIZE", "5"),
            ("RIKU_NGINX_CACHE_TIME", "60"),
            ("RIKU_NGINX_CLOUDFLARE_ACL", "true"),
            ("RIKU_NGINX_ALLOW_GIT_FOLDERS", "1"),
            ("RIKU_NGINX_CATCH_ALL", "index.html"),
            ("RIKU_NGINX_CSP", "default-src 'self'; img-src *"),
            ("RIKU_NGINX_INCLUDE_FILE", "extra.inc"),
        ],
        &backends,
    )
    .unwrap();

    assert_eq!(
        route,
        Route {
            domains: vec!["myapp.example".to_string(), "www.myapp.example".to_string()],
            https: HttpsPolicy::AlsoPlainHttp,
            backends: vec![
                RouteBackend::Tcp {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    port: 5001
                },
                RouteBackend::UnixCopy(2)
            ],
            listen: Listen {
                bind: Ipv4Addr::new(192, 0, 2, 10),
                port: 8080,
                ipv6: None
            },
            tls: None,
            static_paths: vec!["/assets".to_string(), "/media".to_string()],
            kind: SiteKind::Portmap,
            cache: Some(Cache {
                prefixes: vec!["/api/".to_string(), "/feed".to_string()],
                time_s: 60,
                size_gb: 5,
                expiry_s: 86400,
                redirects_s: 3600,
                any_s: 3600,
                control_s: 3600
            }),
            cloudflare_acl: true,
            allow_git_folders: true,
            catch_all: Some("index.html".to_string()),
            csp: Some("default-src 'self'; img-src *".to_string()),
            snippet: Some("add_header X-Frame-Options DENY;\n".to_string()),
        }
    );
}

#[test]
fn site_kind_follows_the_template_order() {
    let kind = |pairs: &[(&str, &str)]| site_kind(&env(pairs), &Router::Nginx);

    assert_eq!(
        kind(&[("RIKU_NGINX_WSGI", "1"), ("RIKU_NGINX_PORTMAP", "1")]),
        SiteKind::Wsgi
    );
    assert_eq!(
        kind(&[("RIKU_NGINX_PORTMAP", "1"), ("RIKU_NGINX_STATIC", "x")]),
        SiteKind::Portmap
    );
    assert_eq!(kind(&[("RIKU_NGINX_STATIC", "x")]), SiteKind::Static);
    assert_eq!(kind(&[]), SiteKind::Plain);
}

#[test]
fn https_only_carries_the_normalized_certificate_pair() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    fs::write(paths.nginx_root.join("myapp.crt"), "leaf").unwrap();
    fs::write(paths.nginx_root.join("myapp.fullchain.crt"), CERT).unwrap();
    fs::write(paths.nginx_root.join("myapp.key"), KEY).unwrap();

    let route = build(
        &tmp,
        &[
            ("RIKU_DOMAINS", "myapp.example"),
            ("RIKU_HTTPS_ONLY", "true"),
            ("RIKU_NGINX_WSGI", "true"),
            ("RIKU_NGINX_EXTERNAL_PORT", "8080"),
        ],
        &[Backend::Unix(paths.socket_root.join("myapp@1.sock"))],
    )
    .unwrap();

    assert_eq!(
        route.tls,
        Some(Tls {
            cert_pem: CERT.to_string(),
            key_pem: KEY.to_string()
        })
    );
    assert_eq!(
        route.listen.port, 80,
        "https_only ignores the external port"
    );
    assert_eq!(route.kind, SiteKind::Wsgi);
    assert!(route.cache.is_none());
}

#[test]
fn a_certificate_without_its_key_is_an_error() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    fs::write(paths.nginx_root.join("myapp.crt"), "leaf").unwrap();

    let err = build(
        &tmp,
        &[
            ("RIKU_DOMAINS", "myapp.example"),
            ("RIKU_HTTPS_ONLY", "true"),
        ],
        &[Backend::loopback(5001)],
    )
    .unwrap_err();

    assert!(err.to_string().contains("only one of"), "{err}");
}

#[test]
fn keys_that_name_a_path_are_refused_by_name() {
    let tmp = TempDir::new().unwrap();
    for key in ["RIKU_NGINX_CACHE_PATH", "RIKU_NGINX_SOCKET"] {
        let err = build(
            &tmp,
            &[("RIKU_DOMAINS", "myapp.example"), (key, "/etc/cron.d")],
            &[Backend::loopback(5001)],
        )
        .unwrap_err();

        assert!(err.to_string().contains(key), "{key}: {err}");
    }
}

#[test]
fn only_the_apps_own_sockets_are_accepted() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    let foreign: [PathBuf; 3] = [
        paths.socket_root.join("other@1.sock"),
        paths.socket_root.join("other.sock"),
        tmp.path().join("myapp@1.sock"),
    ];

    for socket in foreign {
        let err = build(
            &tmp,
            &[("RIKU_DOMAINS", "myapp.example")],
            &[Backend::Unix(socket.clone())],
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("not a socket of 'myapp'"),
            "{socket:?}: {err}"
        );
    }

    let route = build(
        &tmp,
        &[("RIKU_NGINX_STATIC", "public")],
        &[Backend::Unix(paths.socket_root.join("myapp.sock"))],
    )
    .unwrap();
    assert_eq!(route.backends, vec![RouteBackend::UnixDefault]);
    assert_eq!(route.domains, vec!["myapp.example.com".to_string()]);
    assert_eq!(route.kind, SiteKind::Static);
}

#[test]
fn catch_all_domain_and_bind_all_address_work_as_before() {
    let tmp = TempDir::new().unwrap();
    let all = Backend::Tcp {
        host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        port: 5001,
    };

    let route = build(
        &tmp,
        &[("RIKU_DOMAINS", "_"), ("RIKU_NGINX_IPV6_ADDRESS", "")],
        &[all],
    )
    .unwrap();

    assert_eq!(route.domains, vec!["_".to_string()]);
    assert_eq!(
        route.backends,
        vec![RouteBackend::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 5001
        }]
    );
    assert_eq!(route.listen.ipv6, None);
}

#[test]
fn caddy_route_keeps_only_what_caddy_uses() {
    let tmp = TempDir::new().unwrap();
    let backends = [Backend::loopback(5001)];

    let none = build_for(&tmp, &Router::Caddy, &[], &backends).unwrap();
    assert!(none.is_none(), "no domain means the site is removed");

    let route = build_for(
        &tmp,
        &Router::Caddy,
        &[
            ("RIKU_DOMAINS", "myapp.example"),
            ("RIKU_NGINX_PORTMAP", "80:5001"),
            ("RIKU_NGINX_EXTERNAL_PORT", "8080"),
            ("RIKU_NGINX_CSP", "default-src 'self'"),
            ("RIKU_NGINX_CLOUDFLARE_ACL", "true"),
            ("RIKU_NGINX_INCLUDE_FILE", "missing.inc"),
        ],
        &backends,
    )
    .unwrap()
    .unwrap();
    assert_eq!(route.listen, Listen::default());
    assert!(route.csp.is_none() && route.snippet.is_none() && !route.cloudflare_acl);

    let err = build_for(
        &tmp,
        &Router::Caddy,
        &[("RIKU_NGINX_STATIC", "public")],
        &backends,
    )
    .unwrap_err();
    assert!(err.to_string().contains("Caddy does not take"), "{err}");
}
