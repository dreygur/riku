use std::fs;

use super::*;

fn host_paths(dir: &std::path::Path) -> RikuPaths {
    let paths = RikuPaths::for_tests(dir);
    fs::create_dir_all(paths.nginx_include_file.parent().unwrap()).unwrap();
    fs::create_dir_all(&paths.nginx_sites_enabled).unwrap();
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
    assert_eq!(
        includes,
        vec![format!("include {}/*.conf;", paths.nginx_root.display())]
    );
}

#[test]
fn setup_removes_old_riku_links_keeps_other_sites_and_installs_the_include() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = host_paths(tmp.path());
    let old_link = paths.nginx_sites_enabled.join("shop.conf");
    std::os::unix::fs::symlink(paths.nginx_root.join("shop.conf"), &old_link).unwrap();
    let default_site = paths.nginx_sites_enabled.join("default");
    std::os::unix::fs::symlink("/etc/nginx/sites-available/default", &default_site).unwrap();

    load_riku_sites(&paths).unwrap();

    assert!(
        old_link.symlink_metadata().is_err(),
        "old link would load shop a second time"
    );
    assert!(
        default_site.symlink_metadata().is_ok(),
        "a site riku does not own was removed"
    );
    assert!(crate::nginx::includes_riku_sites(&paths), "no include");
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
    assert!(err.contains("not root"), "{err}");

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o775)).unwrap();
    let err = check_write_dir(&dir, me).unwrap_err().to_string();
    assert!(err.contains("group or world writable (mode 775)"), "{err}");
}
