use std::os::unix::fs::{symlink, MetadataExt};

use super::*;

#[test]
fn nginx_dirs_lose_links_into_the_home_and_deploy_owned_riku_files() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let sites = tmp.path().join("sites-enabled");
    let conf_d = tmp.path().join("conf.d");
    for dir in [&home, &sites, &conf_d] {
        fs::create_dir(dir).unwrap();
    }
    let me = fs::metadata(&home).unwrap().uid();
    fs::write(home.join("app.conf"), "x").unwrap();
    fs::write(tmp.path().join("elsewhere.conf"), "x").unwrap();

    symlink(home.join("app.conf"), sites.join("app")).unwrap();
    symlink(home.join("gone.conf"), sites.join("dangling")).unwrap();
    symlink(tmp.path().join("elsewhere.conf"), sites.join("other")).unwrap();
    fs::write(conf_d.join("riku-old.conf"), "x").unwrap();
    fs::write(conf_d.join("riku.conf"), "include").unwrap();
    fs::write(conf_d.join("gzip.conf"), "x").unwrap();

    let removed =
        clear_deploy_owned_router_files(&[&sites, &conf_d], &home, me, "riku.conf").unwrap();

    let mut names: Vec<_> = removed
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["app", "dangling", "riku-old.conf"]);
    assert!(sites.join("other").exists(), "unrelated link was removed");
    assert!(
        conf_d.join("riku.conf").exists(),
        "include file was removed"
    );
    assert!(
        conf_d.join("gzip.conf").exists(),
        "unrelated file was removed"
    );
}

#[test]
fn caddy_dir_keeps_only_root_owned_regular_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("caddy");
    fs::create_dir(&dir).unwrap();
    let me = fs::metadata(&dir).unwrap().uid();
    fs::write(dir.join("app.caddy"), "site").unwrap();
    fs::write(tmp.path().join("target"), "x").unwrap();
    symlink(tmp.path().join("target"), dir.join("link.caddy")).unwrap();
    fs::create_dir(dir.join("nested")).unwrap();
    fs::write(dir.join("nested").join("inner.caddy"), "x").unwrap();

    let removed = clear_foreign_entries(&dir, me).unwrap();

    assert_eq!(removed.len(), 2, "removed {removed:?}");
    assert!(dir.join("app.caddy").is_file(), "owned file was removed");
    assert!(
        !dir.join("link.caddy").exists() && fs::symlink_metadata(dir.join("link.caddy")).is_err()
    );
    assert!(!dir.join("nested").exists());
    assert!(
        tmp.path().join("target").exists(),
        "link target was removed"
    );

    assert!(clear_foreign_entries(&tmp.path().join("missing"), me)
        .unwrap()
        .is_empty());
}
