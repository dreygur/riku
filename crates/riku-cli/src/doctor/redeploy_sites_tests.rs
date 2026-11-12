use std::fs;

use super::*;
use crate::doctor::Status;

#[test]
fn redeploys_only_apps_without_a_site_file_and_reports_them() {
    let tmp = tempfile::tempdir().unwrap();
    let riku_root = tmp.path().join("riku");
    let site_root = tmp.path().join("etc/caddy");
    fs::create_dir_all(&site_root).unwrap();
    for app in ["old", "moved", "broken", "-x"] {
        fs::create_dir_all(riku_root.join("apps").join(app)).unwrap();
    }
    fs::write(riku_root.join("apps").join("stray-file"), "").unwrap();
    fs::write(site_root.join("moved.caddy"), "").unwrap();
    fs::write(site_root.join("old.conf"), "").unwrap();
    let mut seen = Vec::new();

    let checks = redeploy_unrouted_sites(
        &riku_root,
        &site_root,
        "caddy",
        |_| true,
        |app| {
            seen.push(app.to_string());
            match app {
                "broken" => Err(anyhow::anyhow!("no such app")),
                _ => Ok(()),
            }
        },
    );

    assert_eq!(
        seen,
        ["broken", "old"],
        "each unrouted app once; no routed app, plain file or bad name"
    );
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].status, Status::Fail);
    assert!(
        checks[0].detail.contains("no such app"),
        "{}",
        checks[0].detail
    );
    assert_eq!(checks[1].status, Status::Ok);
    assert!(checks[1].detail.contains("old"), "{}", checks[1].detail);
}

#[test]
fn an_app_meant_to_have_no_site_is_not_redeployed_on_every_run() {
    let tmp = tempfile::tempdir().unwrap();
    let riku_root = tmp.path().join("riku");
    let site_root = tmp.path().join("etc/caddy");
    fs::create_dir_all(&site_root).unwrap();
    for app in ["shop", "old"] {
        fs::create_dir_all(riku_root.join("apps").join(app)).unwrap();
    }
    let mut seen = Vec::new();

    let checks = redeploy_unrouted_sites(
        &riku_root,
        &site_root,
        "caddy",
        |app| app != "shop",
        |app| {
            seen.push(app.to_string());
            Ok(())
        },
    );

    assert_eq!(
        seen,
        ["old"],
        "shop is in path mode and has no site of its own"
    );
    assert_eq!(checks.len(), 1);
}

#[test]
fn nothing_is_redeployed_when_the_site_root_is_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    if nix::unistd::Uid::effective().is_root() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let riku_root = tmp.path().join("riku");
    let site_root = tmp.path().join("etc/caddy");
    fs::create_dir_all(&site_root).unwrap();
    fs::create_dir_all(riku_root.join("apps").join("shop")).unwrap();
    fs::write(site_root.join("shop.caddy"), "").unwrap();
    fs::set_permissions(&site_root, fs::Permissions::from_mode(0o000)).unwrap();
    let mut seen = Vec::new();

    let checks = redeploy_unrouted_sites(
        &riku_root,
        &site_root,
        "caddy",
        |_| true,
        |app| {
            seen.push(app.to_string());
            Ok(())
        },
    );

    fs::set_permissions(&site_root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        seen.is_empty(),
        "redeployed {seen:?} without knowing their site is missing"
    );
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Warn, "{}", checks[0].detail);
}
