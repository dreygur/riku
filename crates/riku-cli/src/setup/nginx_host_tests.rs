use std::fs;

use super::*;

fn host_paths(dir: &std::path::Path) -> RikuPaths {
    let mut paths = RikuPaths::for_tests(dir);
    // `nginx -T` must not read the machine's own nginx config.
    paths.reload_programs.nginx = dir.join("no-nginx");
    fs::create_dir_all(paths.nginx_include_file.parent().unwrap()).unwrap();
    paths
}

#[test]
fn include_is_written_once_and_left_alone_on_a_second_run() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = host_paths(tmp.path());

    assert!(install_include(&paths).unwrap(), "first run wrote nothing");
    assert!(
        !install_include(&paths).unwrap(),
        "second run rewrote the file"
    );

    let text = fs::read_to_string(&paths.nginx_include_file).unwrap();
    let includes: Vec<_> = text.lines().filter(|l| l.starts_with("include")).collect();
    assert_eq!(includes, vec!["include /etc/riku/nginx/*.conf;"]);
}

#[test]
fn site_root_is_created_under_a_checked_parent_and_refused_when_writable() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let tmp = tempfile::tempdir().unwrap();
    let parent = tmp.path().join("riku");
    let site = parent.join("nginx");
    let me = fs::metadata(tmp.path()).unwrap().uid();

    prepare_site_root(&site, me).unwrap();
    assert_eq!(fs::metadata(&site).unwrap().mode() & 0o777, 0o755);
    prepare_site_root(&site, me).unwrap();

    fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
    let err = prepare_site_root(&site, me).unwrap_err().to_string();
    assert!(err.contains("group or world writable"), "{err}");
}

#[test]
fn root_write_refuses_a_writable_or_foreign_owned_directory() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("conf.d");
    fs::create_dir(&dir).unwrap();
    let me = fs::metadata(&dir).unwrap().uid();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(check_write_dir(&dir, me).is_ok(), "a 0755 dir of the owner");

    let err = check_write_dir(&dir, me + 1).unwrap_err().to_string();
    assert!(err.contains("owned by uid"), "{err}");

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o775)).unwrap();
    let err = check_write_dir(&dir, me).unwrap_err().to_string();
    assert!(err.contains("group or world writable (mode 775)"), "{err}");
}
