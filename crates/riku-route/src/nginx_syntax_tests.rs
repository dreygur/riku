//! Renders every site kind and has a real nginx check the result. It runs
//! only where `nginx` is on `PATH`; elsewhere it prints a skip line.

use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::process::Command;

use riku_container::types::AppName;
use riku_util::route_plan::HttpsPolicy;
use tempfile::TempDir;

use crate::nginx_render::render;
use crate::nginx_roots::{NginxRoots, ReloadMethod};
use crate::schema::{Cache, Listen, Route, RouteBackend, SiteKind, Tls};
use crate::test_pems::{CERT_A, KEY_A};

fn nginx_available() -> bool {
    Command::new("nginx").arg("-v").output().is_ok()
}

fn roots_in(root: &Path) -> NginxRoots {
    NginxRoots {
        site_root: root.join("sites"),
        cert_root: root.join("certs"),
        cache_root: root.join("cache"),
        socket_root: root.join("sockets"),
        static_root: root.join("static"),
        acme_root: root.join("acme"),
        cloudflare_conf: root.join("cloudflare.conf"),
        nginx: "nginx".into(),
        reload: ReloadMethod::Signal,
        owner_uid: 0,
    }
}

fn base(domain: &str) -> Route {
    Route {
        domains: vec![domain.to_string()],
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

fn mapped(domain: &str, kind: SiteKind, port: u16) -> Route {
    Route {
        kind,
        listen: Listen {
            port,
            ..Listen::default()
        },
        ..base(domain)
    }
}

fn sites() -> Vec<(&'static str, Route)> {
    let plain = Route {
        cache: Some(Cache {
            prefixes: vec!["/api/".to_string()],
            time_s: 60,
            size_gb: 1,
            expiry_s: 600,
            redirects_s: 60,
            any_s: 60,
            control_s: 60,
        }),
        catch_all: Some("index.html".to_string()),
        csp: Some("default-src self".to_string()),
        snippet: Some("add_header X-Frame-Options DENY;".to_string()),
        static_paths: vec!["/assets/".to_string()],
        ..base("plain.example")
    };
    let wsgi = Route {
        backends: vec![RouteBackend::UnixCopy(1)],
        ..mapped("wsgi.example", SiteKind::Wsgi, 8082)
    };
    let fixed = Route {
        backends: Vec::new(),
        static_paths: vec!["/files/".to_string()],
        ..mapped("static.example", SiteKind::Static, 80)
    };
    let secure = Route {
        https: HttpsPolicy::Only,
        tls: Some(Tls {
            cert_pem: CERT_A.to_string(),
            key_pem: KEY_A.to_string(),
        }),
        ..base("secure.example")
    };
    vec![
        ("plainapp", plain),
        (
            "portmapapp",
            mapped("portmap.example", SiteKind::Portmap, 8081),
        ),
        ("wsgiapp", wsgi),
        ("staticapp", fixed),
        ("secureapp", secure),
    ]
}

fn wrapper(root: &Path) -> String {
    let root = root.display();
    format!(
        "pid {root}/nginx.pid;\nerror_log {root}/error.log;\nevents {{ worker_connections 16; }}\nhttp {{\n  access_log off;\n  client_body_temp_path {root}/tmp/body;\n  proxy_temp_path {root}/tmp/proxy;\n  fastcgi_temp_path {root}/tmp/fastcgi;\n  uwsgi_temp_path {root}/tmp/uwsgi;\n  scgi_temp_path {root}/tmp/scgi;\n  include {root}/sites/*.conf;\n}}\n"
    )
}

#[test]
fn every_site_kind_passes_nginx_t() {
    if !nginx_available() {
        eprintln!("SKIPPED every_site_kind_passes_nginx_t: nginx is not on PATH");
        return;
    }
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let roots = roots_in(root);
    fs::create_dir_all(&roots.site_root).unwrap();
    fs::create_dir_all(root.join("tmp")).unwrap();
    fs::create_dir_all(&roots.cache_root).unwrap();
    for (name, route) in sites() {
        let app = AppName::try_from(name).unwrap();
        let cert_dir = roots.cert_dir(&app);
        fs::create_dir_all(&cert_dir).unwrap();
        fs::write(cert_dir.join("fullchain.pem"), CERT_A).unwrap();
        fs::write(cert_dir.join("key.pem"), KEY_A).unwrap();
        let text = render(&roots, &app, &route).unwrap();
        fs::write(roots.site_file(&app), text).unwrap();
    }
    fs::write(root.join("nginx.conf"), wrapper(root)).unwrap();

    let output = Command::new("nginx")
        .arg("-t")
        .arg("-p")
        .arg(root)
        .arg("-c")
        .arg(root.join("nginx.conf"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "nginx -t refused the sites:\n{stderr}"
    );
}
