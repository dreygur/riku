use super::*;
use crate::wire::EntryKind;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

fn set_mode(path: &std::path::Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn scan_records_files_dirs_and_symlinks_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::write(root.join("app.py"), "hello\n").unwrap();
    set_mode(&root.join("app.py"), 0o644);
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("bin/run"), "#!/bin/sh\n").unwrap();
    set_mode(&root.join("bin/run"), 0o755);
    fs::create_dir_all(root.join("venv/bin")).unwrap();
    symlink("/usr/bin/python3", root.join("venv/bin/python")).unwrap();
    fs::create_dir(root.join("logs")).unwrap();
    set_mode(&root.join("logs"), 0o750);
    set_mode(root, 0o710);

    let manifest = scan(root).unwrap();
    assert_eq!(manifest.root_mode, 0o710);
    let wire = serde_json::to_string(&manifest).unwrap();
    let reparsed: crate::wire::Manifest = serde_json::from_str(&wire)
        .unwrap_or_else(|e| panic!("scan output fails manifest validation: {e}"));
    assert_eq!(reparsed, manifest);

    let got: Vec<(&str, &EntryKind)> = manifest
        .entries
        .iter()
        .map(|e| (e.path.as_str(), &e.kind))
        .collect();

    let run_hash = got
        .iter()
        .find(|(p, _)| *p == "bin/run")
        .map(|(_, k)| (*k).clone());
    assert!(matches!(
        run_hash,
        Some(EntryKind::File {
            size: 10,
            mode: 0o755,
            ..
        })
    ));
    let paths: Vec<&str> = got.iter().map(|(p, _)| *p).collect();
    assert_eq!(
        paths,
        [
            "app.py",
            "bin",
            "bin/run",
            "logs",
            "venv",
            "venv/bin",
            "venv/bin/python"
        ]
    );
    assert_eq!(
        got[0].1,
        &EntryKind::File {
            size: 6,
            sha256: "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03".into(),
            mode: 0o644,
        }
    );
    assert_eq!(got[3].1, &EntryKind::Dir { mode: 0o750 });
    assert_eq!(
        got[6].1,
        &EntryKind::Symlink {
            target: "/usr/bin/python3".into()
        }
    );
}

#[test]
fn scan_does_not_follow_symlinked_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "x").unwrap();
    symlink(outside.path(), tmp.path().join("link")).unwrap();

    let manifest = scan(tmp.path()).unwrap();
    assert_eq!(
        manifest.entries.len(),
        1,
        "descended into the link: {manifest:?}"
    );
    assert!(matches!(
        manifest.entries[0].kind,
        EntryKind::Symlink { .. }
    ));
}

#[test]
fn scan_refuses_a_fifo_naming_its_path() {
    let tmp = tempfile::tempdir().unwrap();
    let fifo = tmp.path().join("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success(), "mkfifo failed");

    let err = scan(tmp.path()).unwrap_err().to_string();
    assert!(err.contains("pipe"), "error does not name the path: {err}");
}
