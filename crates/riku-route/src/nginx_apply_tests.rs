use std::collections::BTreeSet;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use riku_config::RikuPaths;
use riku_util::route_plan::HttpsPolicy;
use tempfile::TempDir;

use super::*;
use crate::nginx_roots::ReloadMethod;
use crate::schema::{Listen, RouteBackend, SiteKind};
use crate::test_pems::{CERT_A, KEY_A};

struct Host {
    dir: TempDir,
    roots: NginxRoots,
}

impl Host {
    fn new() -> Host {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let nginx = root.join("mock_nginx");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> {calls}\ncase \"$1\" in\n  -t) if grep -rqs bad.example {sites}; then echo 'emerg: bad directive' >&2; exit 1; fi ;;\nesac\nexit 0\n",
            calls = root.join("calls.log").display(),
            sites = root.join("etc/nginx").display(),
        );
        fs::write(&nginx, script).unwrap();
        fs::set_permissions(&nginx, fs::Permissions::from_mode(0o755)).unwrap();
        let paths = RikuPaths::for_tests(root);
        fs::create_dir_all(&paths.static_root).unwrap();
        let roots = NginxRoots {
            site_root: root.join("etc/nginx"),
            cert_root: root.join("etc/certs"),
            socket_root: paths.socket_root.clone(),
            static_root: paths.static_root.clone(),
            acme_root: paths.acme_root.clone(),
            nginx,
            reload: ReloadMethod::Signal,
            ipv6: true,
            owner_uid: fs::metadata(root).unwrap().uid(),
        };
        Host { dir, roots }
    }

    fn calls(&self) -> String {
        fs::read_to_string(self.dir.path().join("calls.log")).unwrap_or_default()
    }

    fn conf(&self, app: &AppName) -> String {
        fs::read_to_string(self.roots.site_file(app)).unwrap()
    }
}

fn app(name: &str) -> AppName {
    AppName::try_from(name).unwrap()
}

fn route(domain: &str) -> Route {
    Route {
        domains: vec![domain.to_string()],
        https: HttpsPolicy::AlsoPlainHttp,
        backends: vec![RouteBackend::UnixCopy(1)],
        listen: Listen::default(),
        tls: None,
        static_paths: Vec::new(),
        kind: SiteKind::Plain,
        allow_git_folders: false,
        open_port: None,
    }
}

fn https_route(domain: &str) -> Route {
    Route {
        https: HttpsPolicy::Only,
        tls: Some(Tls {
            cert_pem: CERT_A.to_string(),
            key_pem: KEY_A.to_string(),
        }),
        backends: vec![RouteBackend::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 5000,
        }],
        ..route(domain)
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn failure(reply: Reply) -> (Stage, String) {
    match reply {
        Reply::Failed { stage, error } => (stage, error),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn every_route_field_reaches_the_rendered_site() {
    let host = Host::new();
    let myapp = app("myapp");
    let mut sent = route("myapp.example");
    sent.backends = vec![RouteBackend::UnixDefault];
    sent.static_paths = vec!["/assets/".to_string()];
    let reply = apply_nginx(&host.roots, &myapp, &sent);
    assert_eq!(reply, Reply::Ok { reloaded: true }, "apply failed");

    let rendered = host.conf(&myapp);
    let roots = &host.roots;
    let expected = [
        "server_name         myapp.example;".to_string(),
        format!("server unix:{}/myapp.sock;", roots.socket_root.display()),
        format!("alias {}/myapp/paths/0;", roots.static_root.display()),
    ];
    for fragment in expected {
        assert!(
            rendered.contains(&fragment),
            "{fragment:?} missing from:\n{rendered}"
        );
    }
}

#[test]
fn fallback_pair_is_written_for_a_plain_and_tls_site() {
    let host = Host::new();
    let myapp = app("myapp");
    let route = Route {
        https: HttpsPolicy::AlsoPlainHttp,
        ..https_route("myapp.example")
    };

    let reply = apply_nginx(&host.roots, &myapp, &route);

    assert_eq!(reply, Reply::Ok { reloaded: true }, "apply failed");
    let cert_dir = host.roots.cert_dir(&myapp);
    assert!(
        cert_dir.join("fullchain.pem").exists(),
        "the site names a certificate that was never written"
    );
    assert!(host.conf(&myapp).contains("443 ssl"));
}

#[test]
fn failed_bind_restores_old_site() {
    let host = Host::new();
    let myapp = app("myapp");
    apply_nginx(&host.roots, &myapp, &route("myapp.example"));
    let before = host.conf(&myapp);
    let config = render(&host.roots, &myapp, &route("other.example")).unwrap();
    let planned = plan(&host.roots, &myapp, Some(config), None);

    let reply = commit(&host.roots, &planned, Some(8100), BTreeSet::new);

    let (stage, error) = failure(reply);
    assert_eq!(stage, Stage::Reload);
    assert!(error.contains("8100"), "{error}");
    assert_eq!(host.conf(&myapp), before, "the old site must be back");
}

#[test]
fn tls_pair_is_written_with_private_key() {
    let host = Host::new();
    let myapp = app("myapp");

    let reply = apply_nginx(&host.roots, &myapp, &https_route("myapp.example"));

    assert_eq!(reply, Reply::Ok { reloaded: true }, "apply failed");
    let cert_dir = host.roots.cert_dir(&myapp);
    assert_eq!(mode(&cert_dir), 0o700);
    assert_eq!(mode(&cert_dir.join("key.pem")), 0o600);
    assert_eq!(mode(&cert_dir.join("fullchain.pem")), 0o644);
    assert_eq!(mode(&host.roots.site_file(&myapp)), 0o644);
    let conf = host.conf(&myapp);
    let expected = format!(
        "ssl_certificate     {}",
        cert_dir.join("fullchain.pem").display()
    );
    assert!(
        conf.contains(&expected),
        "config names another certificate:\n{conf}"
    );
}

#[test]
fn check_failure_restores_files() {
    let host = Host::new();
    let myapp = app("myapp");
    apply_nginx(&host.roots, &myapp, &https_route("myapp.example"));
    let conf_before = host.conf(&myapp);
    let key_before = fs::read(host.roots.cert_dir(&myapp).join("key.pem")).unwrap();

    let reply = apply_nginx(&host.roots, &myapp, &route("bad.example"));

    let (stage, error) = failure(reply);
    assert_eq!(stage, Stage::Check);
    assert!(
        error.contains("bad directive"),
        "lost nginx's message: {error}"
    );
    assert_eq!(host.conf(&myapp), conf_before, "refused config stayed live");
    let key_after = fs::read(host.roots.cert_dir(&myapp).join("key.pem")).unwrap();
    assert_eq!(key_after, key_before, "certificate key was not restored");
    assert_eq!(
        host.calls().matches("-s reload").count(),
        1,
        "nginx reloaded again after refusing the config"
    );
}

#[test]
fn symlinked_cert_dir_is_refused_and_target_untouched() {
    let host = Host::new();
    let outside = host.dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o750)).unwrap();
    fs::create_dir_all(&host.roots.cert_root).unwrap();
    std::os::unix::fs::symlink(&outside, host.roots.cert_dir(&app("myapp"))).unwrap();

    let reply = apply_nginx(&host.roots, &app("myapp"), &https_route("myapp.example"));

    let (stage, error) = failure(reply);
    assert_eq!(stage, Stage::Render);
    assert!(error.contains("not a directory"), "{error}");
    assert_eq!(mode(&outside), 0o750, "the link target was chmodded");
    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        0,
        "files landed outside"
    );
}

#[test]
fn directory_of_another_uid_is_refused() {
    let mut host = Host::new();
    host.roots.owner_uid += 1;

    let reply = apply_nginx(&host.roots, &app("myapp"), &route("myapp.example"));

    let (stage, error) = failure(reply);
    assert_eq!(stage, Stage::Render);
    assert!(error.contains("owned by uid"), "{error}");
    assert!(!host.roots.site_file(&app("myapp")).exists());
}

#[test]
fn remove_refuses_a_site_root_others_can_write() {
    let host = Host::new();
    let myapp = app("myapp");
    apply_nginx(&host.roots, &myapp, &route("myapp.example"));
    fs::set_permissions(&host.roots.site_root, fs::Permissions::from_mode(0o775)).unwrap();

    let (stage, error) = failure(remove_nginx(&host.roots, &myapp));

    assert_eq!(stage, Stage::Render);
    assert!(
        error.contains(&host.roots.site_root.display().to_string()) && error.contains("755"),
        "the error names the dir and the expected mode: {error}"
    );
    assert!(host.roots.site_file(&myapp).exists(), "a file was removed");
    assert_eq!(
        host.calls().matches("-s reload").count(),
        1,
        "nginx reloaded"
    );
}

#[test]
fn a_cert_root_others_can_write_stops_any_reload() {
    let host = Host::new();
    let myapp = app("myapp");
    apply_nginx(&host.roots, &myapp, &https_route("myapp.example"));
    fs::set_permissions(&host.roots.cert_root, fs::Permissions::from_mode(0o770)).unwrap();

    let (_, error) = failure(apply_nginx(
        &host.roots,
        &app("other"),
        &route("other.example"),
    ));

    assert!(
        error.contains(&host.roots.cert_root.display().to_string()) && error.contains("700"),
        "{error}"
    );
    assert_eq!(
        host.calls().matches("-s reload").count(),
        1,
        "nginx reloaded"
    );
}

#[test]
fn route_the_gate_refuses_is_never_written() {
    let host = Host::new();
    let mut sent = route("myapp.example");
    sent.domains = vec!["myapp.example;".to_string()];

    let reply = apply_nginx(&host.roots, &app("myapp"), &sent);

    let (stage, error) = failure(reply);
    assert_eq!(stage, Stage::Schema);
    assert!(error.contains("myapp.example;"), "{error}");
    assert!(
        !host.roots.site_root.exists(),
        "a site directory was created"
    );
    assert!(host.calls().is_empty(), "nginx ran for a refused route");
}

#[test]
fn remove_takes_app_out_and_reloads_once() {
    let host = Host::new();
    let myapp = app("myapp");
    apply_nginx(&host.roots, &myapp, &https_route("myapp.example"));

    let removed = remove_nginx(&host.roots, &myapp);
    let again = remove_nginx(&host.roots, &myapp);

    assert_eq!(removed, Reply::Ok { reloaded: true });
    assert!(!host.roots.site_file(&myapp).exists());
    assert!(!host.roots.cert_dir(&myapp).exists(), "certificates stayed");
    assert_eq!(again, Reply::Ok { reloaded: false });
    let reloads = host.calls().matches("-s reload").count();
    assert_eq!(reloads, 2, "one reload for the apply, one for the remove");
}
