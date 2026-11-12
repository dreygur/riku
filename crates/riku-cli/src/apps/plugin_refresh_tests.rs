use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::super::bundled_record::{load, sha256_hex};
use super::super::embedded_plugins::{find, write_out};
use super::super::plugin_write::write_plugin_file;
use super::*;

const OLD_PYTHON: &[u8] = b"#!/bin/sh\necho '-----> uv not found'\n";

/// Install `name` the way riku does, then put `old` in its place and in the
/// record, as an older riku would have left it.
fn install_older_copy(root: &Path, name: &str, old: &[u8]) {
    write_out(find(name).unwrap(), root).unwrap();
    write_plugin_file(root, name, old, 0o755).unwrap();
    let record = format!("{name} {}\n", sha256_hex(old));
    write_plugin_file(root, ".bundled", record.as_bytes(), 0o600).unwrap();
}

#[test]
fn a_fresh_install_is_current_and_reported_nowhere() {
    let tmp = tempfile::tempdir().unwrap();
    write_out(find("python").unwrap(), tmp.path()).unwrap();

    let report = refresh_bundled(tmp.path()).unwrap();

    assert_eq!(report, RefreshReport::default());
}

#[test]
fn refreshes_an_unmodified_older_copy() {
    let tmp = tempfile::tempdir().unwrap();
    install_older_copy(tmp.path(), "python", OLD_PYTHON);

    let report = refresh_bundled(tmp.path()).unwrap();

    assert_eq!(report.refreshed, ["python"], "{report:?}");
    let python = tmp.path().join("python");
    let embedded = find("python").unwrap().files[0].bytes;
    assert_eq!(
        fs::read(&python).unwrap(),
        embedded,
        "python kept its old text"
    );
    assert_eq!(
        fs::metadata(&python).unwrap().permissions().mode() & 0o777,
        0o755
    );
    let recorded = load(tmp.path()).unwrap();
    assert_eq!(
        recorded.hash_of("python"),
        Some(sha256_hex(embedded).as_str())
    );
}

#[test]
fn leaves_a_modified_copy_alone_and_names_it() {
    let tmp = tempfile::tempdir().unwrap();
    write_out(find("node").unwrap(), tmp.path()).unwrap();
    fs::write(tmp.path().join("node"), "#!/bin/sh\necho edited\n").unwrap();

    let report = refresh_bundled(tmp.path()).unwrap();

    assert_eq!(report.modified, ["node"], "{report:?}");
    assert!(report.refreshed.is_empty(), "{report:?}");
    assert_eq!(
        fs::read_to_string(tmp.path().join("node")).unwrap(),
        "#!/bin/sh\necho edited\n"
    );
}

#[test]
fn treats_a_copy_without_a_record_as_modified() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("go"), OLD_PYTHON).unwrap();

    let report = refresh_bundled(tmp.path()).unwrap();

    assert_eq!(report.modified, ["go"], "{report:?}");
    assert_eq!(fs::read(tmp.path().join("go")).unwrap(), OLD_PYTHON);
}

#[test]
fn never_follows_a_symlinked_plugin() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plugins");
    install_older_copy(&root, "python", OLD_PYTHON);
    let elsewhere = tmp.path().join("elsewhere");
    fs::write(&elsewhere, OLD_PYTHON).unwrap();
    fs::remove_file(root.join("python")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join("python")).unwrap();

    let report = refresh_bundled(&root).unwrap();

    assert_eq!(report.refused.len(), 1, "{report:?}");
    assert_eq!(report.refused[0].0, "python");
    assert!(report.refreshed.is_empty(), "{report:?}");
    assert_eq!(
        fs::read(&elsewhere).unwrap(),
        OLD_PYTHON,
        "the link target was written"
    );
    assert!(fs::symlink_metadata(root.join("python"))
        .unwrap()
        .file_type()
        .is_symlink());
}
