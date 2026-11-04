use std::fs;

use super::*;
use crate::test_support::{paths_with_nginx, ACCEPT};

#[test]
fn a_host_without_the_nginx_program_is_not_checked_and_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = RikuPaths::for_tests(tmp.path());
    paths.reload_programs.nginx = tmp.path().join("no-nginx-here");

    match check_host_ready(&paths) {
        Ok(HostReadiness::NoNginx(reason)) => {
            assert!(reason.contains("nginx program not found"), "{reason}")
        }
        other => panic!("expected NoNginx, got {other:?}"),
    }
}

#[test]
fn a_conf_that_is_no_apps_config_blocks_installs_by_name() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_nginx(tmp.path(), ACCEPT);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::write(paths.nginx_root.join("shop.conf"), "server {}\n").unwrap();
    fs::write(
        paths.nginx_root.join("acme-challenge.conf.disabled"),
        "server {}\n",
    )
    .unwrap();
    assert_eq!(check_host_ready(&paths).unwrap(), HostReadiness::Ready);

    fs::write(paths.nginx_root.join("ghost.conf"), "server {}\n").unwrap();

    let err = check_host_ready(&paths).unwrap_err().to_string();
    assert!(err.contains("ghost.conf"), "{err}");
}
