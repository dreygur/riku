use std::fs;

use super::*;
use crate::doctor::Status;

fn paths_with_router(router: &str) -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        &paths.settings_file,
        format!("[router]\nname = \"{router}\"\n"),
    )
    .unwrap();
    (tmp, paths)
}

#[test]
fn nginx_without_acme_sh_warns_with_the_install_command() {
    let (_tmp, paths) = paths_with_router("nginx");

    let check = acme_sh(&paths).expect("nginx needs acme.sh");
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(check.detail.contains("get.acme.sh"), "{}", check.detail);

    fs::create_dir_all(&paths.acme_sh_root).unwrap();
    fs::write(paths.acme_sh_root.join("acme.sh"), "#!/bin/sh\n").unwrap();
    assert_eq!(acme_sh(&paths).map(|check| check.status), Some(Status::Ok));
}

#[test]
fn caddy_needs_no_acme_sh() {
    let (_tmp, paths) = paths_with_router("caddy");

    assert!(acme_sh(&paths).is_none());
}
