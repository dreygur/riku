use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::engine::fake::fake_engine;

fn search_path(dirs: &[&std::path::Path]) -> std::ffi::OsString {
    std::env::join_paths(dirs).unwrap()
}

#[test]
fn picks_first_known_engine_on_path() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    fake_engine(first.path(), "podman", "exit 0");
    let docker = fake_engine(second.path(), "docker", "exit 0");

    let engine = detect(None, &search_path(&[first.path(), second.path()])).unwrap();

    assert_eq!(engine.binary_name, "docker");
    assert_eq!(engine.program, docker);
}

#[test]
fn setting_names_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    fake_engine(dir.path(), "docker", "exit 0");
    let nerdctl = fake_engine(dir.path(), "nerdctl", "exit 0");

    let engine = detect(Some("nerdctl"), &search_path(&[dir.path()])).unwrap();

    assert_eq!(engine.binary_name, "nerdctl");
    assert_eq!(engine.program, nerdctl);
}

#[test]
fn setting_may_be_an_absolute_path_with_any_name() {
    let dir = tempfile::tempdir().unwrap();
    let custom = fake_engine(dir.path(), "my-runtime", "exit 0");

    let engine = detect(custom.to_str(), OsStr::new("")).unwrap();

    assert_eq!(engine.binary_name, "my-runtime");
    assert_eq!(engine.program, custom);
}

#[test]
fn setting_that_is_not_executable_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("notexec");
    fs::write(&plain, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
    let value = plain.to_str().unwrap();

    let err = detect(Some(value), OsStr::new("")).unwrap_err().to_string();

    assert!(err.contains("engine.program"), "{err}");
    assert!(err.contains(value), "{err}");
}

#[test]
fn setting_with_relative_path_or_whitespace_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    fake_engine(dir.path(), "docker", "exit 0");
    let path = search_path(&[dir.path()]);

    for bad in ["bin/docker", "./docker", "docker --context x"] {
        let err = detect(Some(bad), &path).unwrap_err().to_string();
        assert!(err.contains("engine.program"), "{bad}: {err}");
    }
}

#[test]
fn no_engine_lists_what_was_searched() {
    let empty = tempfile::tempdir().unwrap();

    let err = detect(None, empty.path().as_os_str())
        .unwrap_err()
        .to_string();

    assert!(err.contains("docker, podman, nerdctl, finch"), "{err}");
}
