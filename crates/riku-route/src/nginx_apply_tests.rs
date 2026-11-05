use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use riku_config::RikuPaths;
use riku_util::route_plan::HttpsPolicy;
use tempfile::TempDir;

use super::*;
use crate::nginx_roots::ReloadMethod;
use crate::schema::{Cache, Listen, RouteBackend, SiteKind};
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
            "#!/bin/sh\necho \"$@\" >> {calls}\ncase \"$1\" in\n  -t) if [ -e {fail_host} ] || grep -rqs bad.example {sites}; then echo 'emerg: bad directive' >&2; exit 1; fi ;;\n  -s) if [ -e {fail_reload} ]; then echo 'reload refused' >&2; exit 1; fi ;;\nesac\nexit 0\n",
            calls = root.join("calls.log").display(),
            fail_host = root.join("fail_host").display(),
            fail_reload = root.join("fail_reload").display(),
            sites = root.join("etc/nginx").display(),
        );
        fs::write(&nginx, script).unwrap();
        fs::set_permissions(&nginx, fs::Permissions::from_mode(0o755)).unwrap();
        let paths = RikuPaths::for_tests(root);
        fs::create_dir_all(&paths.static_root).unwrap();
        let roots = NginxRoots {
            site_root: root.join("etc/nginx"),
            cert_root: root.join("etc/certs"),
            cache_root: root.join("var/cache/nginx"),
            socket_root: paths.socket_root.clone(),
            static_root: paths.static_root.clone(),
            acme_root: paths.acme_root.clone(),
            cloudflare_conf: root.join("etc/cloudflare.conf"),
            nginx,
            reload: ReloadMethod::Signal,
            owner_uid: fs::metadata(root).unwrap().uid(),
            cache_total_gb: 100,
        };
        Host { dir, roots }
    }

    fn flag(&self, name: &str) {
        fs::write(self.dir.path().join(name), "").unwrap();
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
        cache: None,
        cloudflare_acl: false,
        allow_git_folders: false,
        catch_all: None,
        csp: None,
        snippet: None,
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

fn cache(size_gb: u32) -> Cache {
    Cache {
        prefixes: vec!["/api/".to_string()],
        time_s: 3600,
        size_gb,
        expiry_s: 86400,
        redirects_s: 3600,
        any_s: 3600,
        control_s: 3600,
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
    sent.cache = Some(cache(1));
    sent.csp = Some("default-src self".to_string());
    sent.catch_all = Some("index.html".to_string());
    sent.snippet = Some("add_header X-Frame-Options DENY;".to_string());
    let reply = apply_nginx(&host.roots, &myapp, &sent);
    assert_eq!(reply, Reply::Ok { reloaded: true }, "apply failed");

    let rendered = host.conf(&myapp);
    let roots = &host.roots;
    let expected = [
        "server_name         myapp.example;".to_string(),
        format!("server unix:{}/myapp.sock;", roots.socket_root.display()),
        format!("alias {}/myapp/paths/0;", roots.static_root.display()),
        format!(
            "proxy_cache_path {} levels=1:2 keys_zone=myapp:10m",
            roots.cache_dir(&myapp).display()
        ),
        "Content-Security-Policy \"default-src self\"".to_string(),
        "try_files $uri /index.html;".to_string(),
        "add_header X-Frame-Options DENY;".to_string(),
    ];
    for fragment in expected {
        assert!(
            rendered.contains(&fragment),
            "{fragment:?} missing from:\n{rendered}"
        );
    }
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
fn refused_host_config_is_reported_before_any_write() {
    let host = Host::new();
    host.flag("fail_host");

    let reply = apply_nginx(&host.roots, &app("myapp"), &route("myapp.example"));

    let (stage, error) = failure(reply);
    assert_eq!(stage, Stage::Check);
    assert!(error.contains("before any change"), "{error}");
    assert!(!host.roots.site_root.exists(), "files were written anyway");
}

#[test]
fn install_failure_restores_files() {
    let host = Host::new();
    let myapp = app("myapp");
    apply_nginx(&host.roots, &myapp, &route("myapp.example"));
    let conf_before = host.conf(&myapp);
    let mut staged = host.roots.site_file(&myapp).into_os_string();
    staged.push(".new");
    fs::create_dir(&staged).unwrap();

    let reply = apply_nginx(&host.roots, &myapp, &https_route("myapp.example"));

    assert_eq!(failure(reply).0, Stage::Render);
    assert_eq!(host.conf(&myapp), conf_before, "config changed");
    let cert_dir = host.roots.cert_dir(&myapp);
    assert!(
        !cert_dir.join("key.pem").exists() && !cert_dir.join("fullchain.pem").exists(),
        "certificate files written before the failure stayed"
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
fn reload_failure_restores_files() {
    let host = Host::new();
    let myapp = app("myapp");
    apply_nginx(&host.roots, &myapp, &route("myapp.example"));
    let conf_before = host.conf(&myapp);
    host.flag("fail_reload");

    let reply = apply_nginx(&host.roots, &myapp, &route("other.example"));

    assert_eq!(failure(reply).0, Stage::Reload);
    assert_eq!(host.conf(&myapp), conf_before);
}

#[test]
fn access_log_snippet_never_written() {
    let host = Host::new();
    let mut sent = route("myapp.example");
    sent.snippet = Some("access_log /etc/cron.d/x;".to_string());

    let reply = apply_nginx(&host.roots, &app("myapp"), &sent);

    let (stage, error) = failure(reply);
    assert_eq!(stage, Stage::Schema);
    assert!(
        error.contains("access_log"),
        "refusal does not name the directive: {error}"
    );
    assert!(
        !host.roots.site_root.exists(),
        "a site directory was created"
    );
    assert!(host.calls().is_empty(), "nginx ran for a refused route");
}

#[test]
fn cache_total_is_capped() {
    let host = Host::new();
    let mut first = route("first.example");
    first.cache = Some(cache(60));
    let mut second = route("second.example");
    second.cache = Some(cache(60));

    let accepted = apply_nginx(&host.roots, &app("first"), &first);
    let refused = apply_nginx(&host.roots, &app("second"), &second);

    assert_eq!(accepted, Reply::Ok { reloaded: true });
    let conf = host.conf(&app("first"));
    assert!(
        conf.contains("keys_zone=first:10m"),
        "zone grew with the cache:\n{conf}"
    );
    assert!(
        conf.contains("max_size=60g"),
        "cache has no disk bound:\n{conf}"
    );
    let cache_line = conf.find("proxy_cache_path").expect("no cache path line");
    let server_block = conf.find("server {").expect("no server block");
    assert!(
        cache_line < server_block,
        "proxy_cache_path is inside server, where nginx refuses it:\n{conf}"
    );
    assert_eq!(failure(refused).0, Stage::Schema);
    assert!(!host.roots.site_file(&app("second")).exists());
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
