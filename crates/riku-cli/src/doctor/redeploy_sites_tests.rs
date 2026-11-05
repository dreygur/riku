use std::fs;

use super::*;
use crate::doctor::Status;

#[test]
fn redeploys_only_apps_missing_from_the_site_root_and_reports_them() {
    let tmp = tempfile::tempdir().unwrap();
    let riku_root = tmp.path().join("riku");
    let site_root = tmp.path().join("etc/nginx");
    fs::create_dir_all(riku_root.join("nginx")).unwrap();
    fs::create_dir_all(&site_root).unwrap();
    for file in ["old.conf", "moved.conf", "broken.conf", "old.key"] {
        fs::write(riku_root.join("nginx").join(file), "").unwrap();
    }
    fs::write(site_root.join("moved.conf"), "").unwrap();
    let mut seen = Vec::new();

    let checks = redeploy_orphaned_sites(&riku_root, &site_root, |app| {
        seen.push(app.to_string());
        match app {
            "broken" => Err(anyhow::anyhow!("no such app")),
            _ => Ok(()),
        }
    });

    assert_eq!(seen, ["broken", "old"], "each orphan once, no other file");
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
