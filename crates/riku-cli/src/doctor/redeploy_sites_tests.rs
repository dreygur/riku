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

    let checks = redeploy_unrouted_sites(&riku_root, &site_root, "caddy", |app| {
        seen.push(app.to_string());
        match app {
            "broken" => Err(anyhow::anyhow!("no such app")),
            _ => Ok(()),
        }
    });

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
