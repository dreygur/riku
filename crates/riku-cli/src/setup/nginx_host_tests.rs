use std::fs;
use std::os::unix::fs::MetadataExt;

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

    let me = fs::metadata(tmp.path()).unwrap().uid();

    assert!(
        install_include(&paths, me).unwrap(),
        "first run wrote nothing"
    );
    assert!(
        !install_include(&paths, me).unwrap(),
        "second run rewrote the file"
    );
    assert!(
        install_include(&paths, me + 1).unwrap(),
        "a file root does not own was kept"
    );

    let text = fs::read_to_string(&paths.nginx_include_file).unwrap();
    let includes: Vec<_> = text.lines().filter(|l| l.starts_with("include")).collect();
    assert_eq!(includes, vec!["include /etc/riku/nginx/*.conf;"]);
}

#[test]
fn site_root_is_created_under_a_checked_parent_and_refused_when_writable() {
    use std::os::unix::fs::PermissionsExt;

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

/// Every init run puts the site root back to `0755`, not only the run that
/// created it.
#[test]
fn an_existing_site_root_others_can_write_gets_its_mode_back() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let site = tmp.path().join("riku").join("nginx");
    fs::create_dir_all(&site).unwrap();
    fs::set_permissions(&site, fs::Permissions::from_mode(0o777)).unwrap();
    let me = fs::metadata(tmp.path()).unwrap().uid();

    prepare_site_root(&site, me).unwrap();

    assert_eq!(fs::metadata(&site).unwrap().mode() & 0o7777, 0o755);
}

#[test]
fn a_symlinked_site_root_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let parent = tmp.path().join("riku");
    let elsewhere = tmp.path().join("elsewhere");
    fs::create_dir_all(&parent).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, parent.join("nginx")).unwrap();
    let me = fs::metadata(tmp.path()).unwrap().uid();

    let err = prepare_site_root(&parent.join("nginx"), me)
        .unwrap_err()
        .to_string();

    assert!(err.contains("not a directory"), "{err}");
}

#[test]
fn root_write_refuses_a_writable_or_foreign_owned_directory() {
    use std::os::unix::fs::PermissionsExt;

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
