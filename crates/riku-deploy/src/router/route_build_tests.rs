use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;
use crate::router::tests::env;
use riku_route::{Listen, Tls};

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
    build_route(&app(), &env(pairs), backends, &paths, router)
}

fn build(tmp: &TempDir, pairs: &[(&str, &str)], backends: &[Backend]) -> Result<Route> {
    build_for(tmp, &Router::Nginx, pairs, backends).map(|route| route.expect("nginx always routes"))
}

#[test]
fn env_maps_to_route() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
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
            ("RIKU_NGINX_STATIC", "public"),
            ("RIKU_NGINX_STATIC_PATHS", "/assets:dist, /media:uploads"),
            ("RIKU_NGINX_ALLOW_GIT_FOLDERS", "1"),
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
            listen: Listen { port: 8080 },
            tls: None,
            static_paths: vec!["/assets".to_string(), "/media".to_string()],
            kind: SiteKind::Portmap,
            allow_git_folders: true,
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
fn removed_and_path_keys_fail_the_deploy_naming_the_key() {
    let tmp = TempDir::new().unwrap();
    for key in ["RIKU_NGINX_CSP", "RIKU_NGINX_SOCKET"] {
        for router in [Router::Nginx, Router::Caddy] {
            let err = build_for(
                &tmp,
                &router,
                &[("RIKU_DOMAINS", "myapp.example"), (key, "x")],
                &[Backend::loopback(5001)],
            )
            .unwrap_err();

            assert!(err.to_string().contains(key), "{key}: {err}");
        }
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

    let route = build(&tmp, &[("RIKU_DOMAINS", "_")], &[all]).unwrap();

    assert_eq!(route.domains, vec!["_".to_string()]);
    assert_eq!(
        route.backends,
        vec![RouteBackend::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 5001
        }]
    );
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
        ],
        &backends,
    )
    .unwrap()
    .unwrap();
    assert_eq!(route.listen, Listen::default());

    let err = build_for(
        &tmp,
        &Router::Caddy,
        &[("RIKU_NGINX_STATIC", "public")],
        &backends,
    )
    .unwrap_err();
    assert!(err.to_string().contains("Caddy does not take"), "{err}");
}
