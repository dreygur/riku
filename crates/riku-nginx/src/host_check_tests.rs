use std::fs;
use std::path::Path;

use super::*;
use crate::host::write_include_file;
use crate::test_support::{paths_with_nginx, ACCEPT};

/// A host with nginx (mocked), the include installed and app `shop`.
fn ready_host(dir: &Path) -> RikuPaths {
    let paths = paths_with_nginx(dir, ACCEPT);
    write_include_file(&paths).unwrap();
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    paths
}

#[test]
fn a_host_without_the_nginx_program_is_not_checked_and_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = RikuPaths::for_tests(tmp.path());
    paths.reload_programs.nginx = tmp.path().join("no-nginx-here");

    match check_host_ready(&paths, RouteChange::Install) {
        Ok(HostReadiness::NoNginx(reason)) => {
            assert!(reason.contains("nginx program not found"), "{reason}")
        }
        other => panic!("expected NoNginx, got {other:?}"),
    }
}

#[test]
fn nginx_without_the_include_directory_fails_naming_it() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_nginx(tmp.path(), ACCEPT);
    let dir = paths.nginx_include_file.parent().unwrap().to_path_buf();

    let err = check_host_ready(&paths, RouteChange::Remove)
        .expect_err("nginx is installed but conf.d is missing")
        .to_string();

    assert!(err.contains(&dir.display().to_string()), "{err}");
    assert!(err.contains("create it"), "{err}");
}

#[test]
fn a_conf_that_is_no_apps_config_blocks_installs_by_name_but_not_removal() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = ready_host(tmp.path());
    fs::write(paths.nginx_root.join("shop.conf"), "server {}\n").unwrap();
    fs::write(
        paths.nginx_root.join(crate::ACME_CONFIG_FILE),
        "server {}\n",
    )
    .unwrap();
    assert_eq!(
        check_host_ready(&paths, RouteChange::Install).unwrap(),
        HostReadiness::Ready
    );

    fs::write(paths.nginx_root.join(STALE_VALIDATION_FILE), "events {}\n").unwrap();

    let err = check_host_ready(&paths, RouteChange::Install)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(STALE_VALIDATION_FILE) && err.contains("sudo riku init"),
        "{err}"
    );
    assert_eq!(
        check_host_ready(&paths, RouteChange::Remove).unwrap(),
        HostReadiness::Ready
    );
}
