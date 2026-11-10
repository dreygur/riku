use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use riku_config::RikuPaths;
use tempfile::TempDir;

use super::*;
use crate::nginx_roots::ReloadMethod;

fn roots(dir: &TempDir) -> NginxRoots {
    let root = dir.path();
    let nginx = root.join("mock_nginx");
    fs::write(&nginx, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&nginx, fs::Permissions::from_mode(0o755)).unwrap();
    let paths = RikuPaths::for_tests(root);
    let site_root = root.join("etc/nginx");
    fs::create_dir_all(&site_root).unwrap();
    NginxRoots {
        site_root,
        cert_root: root.join("etc/certs"),
        socket_root: paths.socket_root.clone(),
        static_root: paths.static_root.clone(),
        acme_root: paths.acme_root.clone(),
        nginx,
        reload: ReloadMethod::Signal,
        ipv6: false,
        owner_uid: fs::metadata(root).unwrap().uid(),
    }
}

#[test]
fn empty_path_list_removes_the_site() {
    let dir = TempDir::new().unwrap();
    let roots = roots(&dir);
    let file = roots.site_root.join(NGINX_FILE);
    fs::write(&file, "server {}\n").unwrap();

    let reply = apply_paths_nginx(&roots, &Listen { port: 8080 }, &[]);

    assert_eq!(reply, Reply::Ok { reloaded: true });
    assert!(!file.exists(), "the path site is still there");
}

#[test]
fn empty_path_list_without_a_site_does_not_reload() {
    let dir = TempDir::new().unwrap();
    let roots = roots(&dir);

    let reply = apply_paths_nginx(&roots, &Listen { port: 8080 }, &[]);

    assert_eq!(reply, Reply::Ok { reloaded: false });
}

#[test]
fn refused_app_name_never_writes_a_file() {
    let dir = TempDir::new().unwrap();
    let roots = roots(&dir);
    let apps = [PathApp {
        app: "_paths".to_string(),
        backends: vec![crate::schema::RouteBackend::UnixCopy(1)],
    }];

    let reply = apply_paths_nginx(&roots, &Listen { port: 8080 }, &apps);

    assert!(
        matches!(
            reply,
            Reply::Failed {
                stage: Stage::Schema,
                ..
            }
        ),
        "{reply:?}"
    );
    assert!(!roots.site_root.join(NGINX_FILE).exists());
}
