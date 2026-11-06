use std::fs;

use super::remove_deploy_files;
use crate::config::RikuPaths;

fn paths_in(dir: &std::path::Path) -> RikuPaths {
    let paths = RikuPaths::for_tests(dir);
    fs::create_dir_all(&paths.nginx_root).unwrap();
    fs::create_dir_all(&paths.socket_root).unwrap();
    paths
}

#[test]
fn removes_the_certificate_files_and_sockets_of_one_app_only() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_in(tmp.path());
    for name in ["myapp.key", "myapp.crt", "myapp.fullchain.crt", "other.key"] {
        fs::write(paths.nginx_root.join(name), "pem").unwrap();
    }
    for name in [
        "myapp.sock",
        "myapp@2.sock",
        "other.sock",
        "myapp-two@1.sock",
    ] {
        fs::write(paths.socket_root.join(name), "").unwrap();
    }

    remove_deploy_files("myapp", &paths).unwrap();
    remove_deploy_files("myapp", &paths).unwrap();

    let mut left: Vec<String> = fs::read_dir(&paths.nginx_root)
        .unwrap()
        .chain(fs::read_dir(&paths.socket_root).unwrap())
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["myapp-two@1.sock", "other.key", "other.sock"]);
}

#[test]
fn every_failure_is_reported_in_one_error() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_in(tmp.path());
    for stuck in [
        paths.socket_root.join("myapp@1.sock"),
        paths.nginx_root.join("myapp.crt"),
    ] {
        fs::create_dir_all(stuck.join("busy")).unwrap();
    }

    let err = remove_deploy_files("myapp", &paths)
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("myapp@1.sock") && err.contains("myapp.crt"),
        "{err}"
    );
}
