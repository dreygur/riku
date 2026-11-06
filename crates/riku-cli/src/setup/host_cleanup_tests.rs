use std::os::unix::fs::{symlink, MetadataExt};

use super::*;

#[test]
fn nginx_dirs_keep_only_root_owned_files_and_links_to_them() {
    use std::os::unix::fs::PermissionsExt;

    // The rule refuses any link or writable directory on the way to a target.
    // /tmp is world writable and macOS keeps temp dirs under the /var link,
    // so work from the real path of a directory inside this build's target.
    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    fs::create_dir_all(&target_dir).unwrap();
    let tmp_dir = tempfile::tempdir_in(&target_dir).unwrap();
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

    // The good link is kept only when every directory above this test's tree
    // is safe too, which depends on where the checkout lives.
    let chain_safe = tmp.ancestors().all(|dir| {
        let meta = fs::symlink_metadata(dir).unwrap();
        meta.is_dir() && (meta.uid() == me || meta.uid() == 0) && meta.mode() & 0o022 == 0
    });

    let first_removed = tmp.join("removed").join("1");
    check_removed_base(&first_removed, me).unwrap();
    let removed = clear_deploy_owned_router_files(&[&sites, &conf_d], me, &first_removed).unwrap();
    let mut names: Vec<_> = removed
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.retain(|name| name != "good-link");
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
    if chain_safe {
        assert!(sites.join("good-link").exists(), "link to a root file went");
    } else {
        println!(
            "SKIPPED good link check: a directory above {} is writable by others",
            tmp.display()
        );
    }
    assert!(conf_d.join("gzip.conf").exists(), "root owned file went");
    assert!(conf_d.join("nested").is_dir(), "directory was removed");
    let kept_link = first_removed.join("sites-enabled").join("dangling");
    assert!(
        fs::symlink_metadata(&kept_link).unwrap().is_symlink(),
        "{} was not moved as a link",
        kept_link.display()
    );
    for dir in [&first_removed, first_removed.parent().unwrap()] {
        let mode = fs::metadata(dir).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o700, "{} is mode {mode:o}", dir.display());
    }

    let second_removed = tmp.join("removed").join("2");
    let removed =
        clear_deploy_owned_router_files(&[&sites, &conf_d], not_root, &second_removed).unwrap();
    let mut names: Vec<_> = removed
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.retain(|name| name != "good-link");
    names.sort();
    assert_eq!(names, ["gzip.conf"], "deploy owned entries stayed");
    assert!(
        !sites.join("good-link").exists(),
        "deploy owned link stayed"
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
