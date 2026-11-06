use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

use nix::unistd::{Gid, Uid};

use super::*;

#[test]
fn import_added_once_and_points_outside_the_home() {
    let home = PathBuf::from("/home/deploy");
    let line = import_line(&RikuPaths::from_dirs(home.join(".riku"), &home));

    let updated = caddyfile_with_import(":80 {\n}\n", &line).expect("import should be added");
    assert!(
        updated.ends_with("}\nimport /etc/riku/caddy/*.caddy\n"),
        "{updated}"
    );
    assert_eq!(
        caddyfile_with_import(&updated, &line),
        None,
        "second run must not add it again"
    );
}

#[test]
fn admin_socket_merged_or_refused() {
    let added = caddyfile_with_admin(":80 {\n}\n").unwrap().unwrap();
    assert!(
        added.starts_with("{\n\tadmin unix//run/caddy/admin.sock\n}\n\n:80"),
        "{added}"
    );

    let merged = caddyfile_with_admin("{\n\temail a@b.c\n}\n:80 {\n}\n")
        .unwrap()
        .unwrap();
    assert!(
        merged.starts_with("{\n\tadmin unix//run/caddy/admin.sock\n\temail"),
        "{merged}"
    );
    assert_eq!(caddyfile_with_admin(&merged).unwrap(), None);

    for bad in ["{ debug }\n:80 {\n}\n", "{\n\tadmin localhost:2019\n}\n"] {
        let err = caddyfile_with_admin(bad).unwrap_err().to_string();
        assert!(
            err.contains("admin unix//run/caddy/admin.sock"),
            "{bad}: {err}"
        );
    }
}

#[test]
fn site_root_is_setgid_and_closed_to_others() {
    let tmp = tempfile::tempdir().unwrap();
    let site_root = tmp.path().join("etc-riku").join("caddy");
    let group = Group::from_gid(Gid::current()).unwrap().unwrap().name;

    prepare_site_root(&site_root, Uid::current().as_raw(), None, &group).unwrap();

    let meta = fs::metadata(&site_root).unwrap();
    assert_eq!(
        meta.permissions().mode() & 0o7777,
        0o2750,
        "site dir mode is {:o}",
        meta.mode()
    );
    assert_eq!(meta.gid(), Gid::current().as_raw(), "site dir group");
}

#[test]
fn refuses_a_parent_others_can_write() {
    let tmp = tempfile::tempdir().unwrap();
    let parent = tmp.path().join("etc-riku");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
    let group = Group::from_gid(Gid::current()).unwrap().unwrap().name;

    let err = prepare_site_root(&parent.join("caddy"), Uid::current().as_raw(), None, &group)
        .unwrap_err()
        .to_string();

    assert!(err.contains("world writable"), "{err}");
    assert!(!parent.join("caddy").exists(), "site dir created anyway");
}
