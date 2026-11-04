use std::fs;

use super::remove_nginx_files;
use crate::test_support::paths_with_nginx;

#[test]
fn removal_reports_a_live_config_and_never_reloads_or_leaves_nginx_root() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let marker = temp_dir.path().join("direct");
    let paths = paths_with_nginx(
        temp_dir.path(),
        &format!("touch {}\nexit 0\n", marker.display()),
    );
    fs::write(paths.nginx_root.join("myapp.conf"), "server {}\n").unwrap();
    fs::write(paths.nginx_root.join("myapp.conf.new"), "server {}\n").unwrap();
    fs::create_dir_all(&paths.nginx_sites_enabled).unwrap();
    let legacy = paths.nginx_sites_enabled.join("myapp.conf");
    std::os::unix::fs::symlink(paths.nginx_root.join("myapp.conf"), &legacy).unwrap();

    assert!(remove_nginx_files("myapp", &paths).unwrap().was_live);
    assert!(!remove_nginx_files("myapp", &paths).unwrap().was_live);

    assert!(
        !marker.exists(),
        "removal reloaded nginx directly, the caller owns the reload"
    );
    assert_eq!(fs::read_dir(&paths.nginx_root).unwrap().count(), 0);
    assert!(
        legacy.symlink_metadata().is_ok(),
        "removal wrote outside nginx_root"
    );
}

#[test]
fn side_file_failures_are_all_reported_and_the_removed_live_config_still_counts() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), "exit 0\n");
    fs::write(paths.nginx_root.join("myapp.conf"), "server {}\n").unwrap();
    for stuck in [
        paths.socket_root.join("myapp@1.sock"),
        paths.nginx_root.join("myapp.crt"),
    ] {
        fs::create_dir_all(stuck.join("busy")).unwrap();
    }

    let removal = remove_nginx_files("myapp", &paths).unwrap();

    assert!(removal.was_live, "the live config went but no reload asked");
    let err = removal
        .leftover_error
        .expect("side files could not be removed but no error came back")
        .to_string();
    assert!(
        err.contains("myapp@1.sock") && err.contains("myapp.crt"),
        "{err}"
    );
    assert!(!paths.nginx_root.join("myapp.conf").exists());
}

#[test]
fn a_dangling_config_link_is_removed_and_counts_as_live() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), "exit 0\n");
    let link = paths.nginx_root.join("myapp.conf");
    std::os::unix::fs::symlink(temp_dir.path().join("gone.conf"), &link).unwrap();

    let removal = remove_nginx_files("myapp", &paths).unwrap();

    assert!(removal.was_live, "nginx still globbed the link");
    assert!(
        link.symlink_metadata().is_err(),
        "dangling link left behind"
    );
}
