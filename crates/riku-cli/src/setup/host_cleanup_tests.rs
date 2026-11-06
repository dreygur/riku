use std::os::unix::fs::{symlink, MetadataExt};

use super::*;

#[test]
fn nginx_dirs_keep_only_root_owned_files_and_links_to_them() {
    use std::os::unix::fs::PermissionsExt;

    // The rule refuses any link on the way, and macOS keeps temp dirs under
    // the /var link, so work from the real path.
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp = fs::canonicalize(tmp_dir.path()).unwrap();
    let sites = tmp.join("sites-enabled");
    let conf_d = tmp.join("conf.d");
    let outside = tmp.join("outside");
    for dir in [&sites, &conf_d, &outside] {
        fs::create_dir(dir).unwrap();
    }
    let me = fs::metadata(&sites).unwrap().uid();
    let not_root = me + 1;
    fs::write(outside.join("shared.conf"), "x").unwrap();
    fs::write(outside.join("writable.conf"), "x").unwrap();
    fs::set_permissions(
        outside.join("writable.conf"),
        fs::Permissions::from_mode(0o666),
    )
    .unwrap();

    symlink(outside.join("shared.conf"), sites.join("good-link")).unwrap();
    symlink(outside.join("writable.conf"), sites.join("writable-link")).unwrap();
    symlink(outside.join("gone.conf"), sites.join("dangling")).unwrap();
    let loose = tmp.join("loose");
    fs::create_dir(&loose).unwrap();
    fs::write(loose.join("site.conf"), "x").unwrap();
    fs::set_permissions(&loose, fs::Permissions::from_mode(0o777)).unwrap();
    symlink(loose.join("site.conf"), sites.join("loose-link")).unwrap();
    symlink(outside.join("shared.conf"), outside.join("hop")).unwrap();
    symlink(outside.join("hop"), sites.join("chained")).unwrap();
    symlink(&outside, tmp.join("alias")).unwrap();
    symlink(
        tmp.join("alias").join("shared.conf"),
        sites.join("through-linked-dir"),
    )
    .unwrap();
    fs::write(conf_d.join("gzip.conf"), "x").unwrap();
    fs::create_dir(conf_d.join("nested")).unwrap();

    let removed = clear_deploy_owned_router_files(&[&sites, &conf_d], me).unwrap();
    let mut names: Vec<_> = removed
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "chained",
            "dangling",
            "loose-link",
            "through-linked-dir",
            "writable-link"
        ]
    );
    assert!(sites.join("good-link").exists(), "link to a root file went");
    assert!(conf_d.join("gzip.conf").exists(), "root owned file went");
    assert!(conf_d.join("nested").is_dir(), "directory was removed");

    let removed = clear_deploy_owned_router_files(&[&sites, &conf_d], not_root).unwrap();
    let mut names: Vec<_> = removed
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["good-link", "gzip.conf"],
        "deploy owned entries stayed"
    );
    assert!(
        outside.join("shared.conf").exists(),
        "link target was removed"
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
