use std::ffi::OsString;
use std::os::unix::fs::MetadataExt;

use nix::unistd::Uid;

use crate::config::host_file::read_host;

use super::*;

fn fake_root(dirs: &[&str]) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for dir in dirs {
        fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    root
}

fn bin_dir_with_openrc_run() -> (tempfile::TempDir, OsString) {
    let bin = tempfile::tempdir().unwrap();
    let runner = bin.path().join(OPENRC_RUNNER);
    fs::write(&runner, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    let path_var = bin.path().as_os_str().to_os_string();
    (bin, path_var)
}

#[test]
fn systemd_dir_means_systemd() {
    let root = fake_root(&[SYSTEMD_RUN_DIR]);
    assert_eq!(
        detect_with(root.path(), OsStr::new("")),
        InitSystem::Systemd
    );
}

#[test]
fn openrc_needs_both_signals() {
    let (_bin, path_var) = bin_dir_with_openrc_run();
    let without_run_dir = fake_root(&[]);
    assert_eq!(
        detect_with(without_run_dir.path(), &path_var),
        InitSystem::Other,
        "binary alone must not count"
    );

    let with_run_dir = fake_root(&[OPENRC_RUN_DIR]);
    assert_eq!(
        detect_with(with_run_dir.path(), OsStr::new("")),
        InitSystem::Other,
        "run dir alone must not count"
    );
    assert_eq!(
        detect_with(with_run_dir.path(), &path_var),
        InitSystem::OpenRc
    );
}

#[test]
fn host_file_is_written_world_readable_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let file = dir.path().join("host.toml");
    let facts = HostFacts {
        init: InitSystem::OpenRc,
    };

    write_host_file(&file, Uid::current().as_raw(), &facts).unwrap();

    assert_eq!(fs::metadata(&file).unwrap().mode() & 0o777, HOST_FILE_MODE);
    assert_eq!(read_host(&file).unwrap(), Some(facts));
}
