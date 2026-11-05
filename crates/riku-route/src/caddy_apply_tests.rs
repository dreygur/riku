use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use riku_util::route_plan::HttpsPolicy;
use tempfile::TempDir;

use super::*;
use crate::schema::{Listen, SiteKind};

struct Host {
    dir: TempDir,
    roots: CaddyRoots,
}

impl Host {
    fn new() -> Host {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let caddy = root.join("mock_caddy");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> {calls}\nif [ -e {fail} ]; then echo 'admin endpoint refused' >&2; exit 1; fi\n",
            calls = root.join("calls.log").display(),
            fail = root.join("fail_reload").display(),
        );
        fs::write(&caddy, script).unwrap();
        fs::set_permissions(&caddy, fs::Permissions::from_mode(0o755)).unwrap();
        let site_root = root.join("etc/caddy");
        fs::create_dir_all(&site_root).unwrap();
        let roots = CaddyRoots {
            site_root,
            caddy,
            caddyfile: root.join("Caddyfile"),
            admin_address: "unix//run/caddy/admin.sock".to_string(),
            owner_uid: fs::metadata(root).unwrap().uid(),
        };
        Host { dir, roots }
    }

    fn refuse_reloads(&self) {
        fs::write(self.dir.path().join("fail_reload"), "").unwrap();
    }

    fn calls(&self) -> String {
        fs::read_to_string(self.dir.path().join("calls.log")).unwrap_or_default()
    }

    fn site(&self, app: &AppName) -> String {
        fs::read_to_string(self.roots.site_file(app)).unwrap()
    }
}

fn app(name: &str) -> AppName {
    AppName::try_from(name).unwrap()
}

fn route(port: u16) -> Route {
    Route {
        domains: vec!["shop.example".to_string()],
        https: HttpsPolicy::Only,
        backends: vec![RouteBackend::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
        }],
        listen: Listen::default(),
        tls: None,
        static_paths: Vec::new(),
        kind: SiteKind::Plain,
        allow_git_folders: false,
    }
}

fn failure(reply: Reply) -> (Stage, String) {
    match reply {
        Reply::Failed { stage, error } => (stage, error),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn site_is_written_group_readable_and_reloaded_over_the_admin_socket() {
    let host = Host::new();
    let shop = app("shop");

    let reply = apply_caddy(&host.roots, &shop, &route(5000));

    assert_eq!(reply, Reply::Ok { reloaded: true });
    assert_eq!(
        host.site(&shop),
        "# riku app: shop\nshop.example {\n\treverse_proxy 127.0.0.1:5000\n}\n"
    );
    let mode = fs::metadata(host.roots.site_file(&shop))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o640);
    let expected = format!(
        "reload --config {} --adapter caddyfile --address unix//run/caddy/admin.sock",
        host.dir.path().join("Caddyfile").display()
    );
    assert_eq!(host.calls().trim(), expected);
}

#[test]
fn caddy_reload_failure_restores() {
    let host = Host::new();
    let shop = app("shop");
    apply_caddy(&host.roots, &shop, &route(5000));
    let before = host.site(&shop);
    host.refuse_reloads();

    let (stage, error) = failure(apply_caddy(&host.roots, &shop, &route(5001)));

    assert_eq!(stage, Stage::Reload);
    assert!(error.contains("admin endpoint refused"), "{error}");
    assert_eq!(host.site(&shop), before, "refused site stayed on disk");
}

#[test]
fn nginx_only_field_for_caddy_is_refused() {
    let host = Host::new();
    let shop = app("shop");
    let with_static = Route {
        static_paths: vec!["/assets/".to_string()],
        ..route(5000)
    };

    let (stage, error) = failure(apply_caddy(&host.roots, &shop, &with_static));

    assert_eq!(stage, Stage::Schema);
    assert!(
        error.contains("Caddy does not take static sites"),
        "{error}"
    );
    assert!(!host.roots.site_file(&shop).exists());
    assert_eq!(host.calls(), "", "caddy ran for a refused route");
}

#[test]
fn remove_takes_the_site_out_and_reloads_once() {
    let host = Host::new();
    let shop = app("shop");
    apply_caddy(&host.roots, &shop, &route(5000));

    let removed = remove_caddy(&host.roots, &shop);
    let again = remove_caddy(&host.roots, &shop);

    assert_eq!(removed, Reply::Ok { reloaded: true });
    assert!(!host.roots.site_file(&shop).exists());
    assert_eq!(again, Reply::Ok { reloaded: false });
    assert_eq!(host.calls().lines().count(), 2, "{}", host.calls());
}
