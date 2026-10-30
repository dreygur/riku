use super::*;

#[test]
fn rel_path_refuses_escapes() {
    for bad in [
        "",
        "/etc/passwd",
        "../x",
        "a/../b",
        "a//b",
        "a/./b",
        "a\\b",
        "a/",
        "a\0b",
    ] {
        assert!(RelPath::try_from(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn rel_path_parent_and_to_path() {
    let path = RelPath::try_from("venv/bin/python").unwrap();
    assert_eq!(path.to_path(), std::path::PathBuf::from("venv/bin/python"));
    assert_eq!(path.parent().unwrap().as_str(), "venv/bin");
    assert!(RelPath::try_from("app.py").unwrap().parent().is_none());
}

#[test]
fn manifest_json_with_escape_fails_because_of_the_path() {
    let raw = r#"{"root_mode":493,"entries":[{"path":"../x","kind":"dir","mode":493}]}"#;
    let err = serde_json::from_str::<Manifest>(raw)
        .unwrap_err()
        .to_string();
    assert!(err.contains("../x"), "error does not name the path: {err}");

    let clean = r#"{"root_mode":493,"entries":[{"path":"x","kind":"dir","mode":493}]}"#;
    let manifest: Manifest = serde_json::from_str(clean).unwrap();
    assert_eq!(manifest.entries[0].path.as_str(), "x");
    assert_eq!(manifest.entries[0].kind, EntryKind::Dir { mode: 0o755 });
}

fn parse_entries(entries: &str) -> Result<Manifest, String> {
    let raw = format!(r#"{{"root_mode":493,"entries":[{entries}]}}"#);
    serde_json::from_str::<Manifest>(&raw).map_err(|e| e.to_string())
}

const DIR_A: &str = r#"{"path":"a","kind":"dir","mode":493}"#;
const DIR_B: &str = r#"{"path":"b","kind":"dir","mode":493}"#;

#[test]
fn manifest_refuses_unsorted_entries_naming_the_path() {
    let err = parse_entries(&format!("{DIR_B},{DIR_A}")).unwrap_err();
    assert!(err.contains("\"a\"") && err.contains("order"), "got: {err}");
}

#[test]
fn manifest_refuses_duplicate_paths() {
    let err = parse_entries(&format!("{DIR_A},{DIR_A}")).unwrap_err();
    assert!(err.contains("\"a\""), "got: {err}");
}

#[test]
fn manifest_refuses_entry_under_a_symlink() {
    let link = r#"{"path":"x","kind":"symlink","target":"/etc"}"#;
    let file = r#"{"path":"x/y","kind":"file","size":1,"sha256":"ab","mode":420}"#;
    let err = parse_entries(&format!("{link},{file}")).unwrap_err();
    assert!(err.contains("x/y"), "got: {err}");
}

#[test]
fn manifest_refuses_entry_whose_parent_is_not_listed() {
    let err = parse_entries(r#"{"path":"a/b","kind":"dir","mode":493}"#).unwrap_err();
    assert!(err.contains("a/b"), "got: {err}");
}

#[test]
fn manifest_refuses_mode_bits_outside_0o777() {
    let file = r#"{"path":"run","kind":"file","size":1,"sha256":"ab","mode":2541}"#;
    let err = parse_entries(file).unwrap_err();
    assert!(err.contains("run") && err.contains("4755"), "got: {err}");
}

#[test]
fn manifest_refuses_directory_and_root_modes_outside_0o777() {
    let setuid_dir = r#"{"path":"bin","kind":"dir","mode":2541}"#;
    let err = parse_entries(setuid_dir).unwrap_err();
    assert!(err.contains("bin") && err.contains("4755"), "got: {err}");

    let sticky_root = r#"{"root_mode":1023,"entries":[]}"#;
    let err = serde_json::from_str::<Manifest>(sticky_root)
        .unwrap_err()
        .to_string();
    assert!(err.contains("root") && err.contains("1777"), "got: {err}");
}

#[test]
fn manifest_accepts_nested_dirs_and_files() {
    let nested = r#"{"path":"a/f","kind":"file","size":1,"sha256":"ab","mode":493}"#;
    let manifest = parse_entries(&format!("{DIR_A},{nested}")).unwrap();
    assert_eq!(manifest.entries.len(), 2);
}

#[test]
fn file_entry_round_trips_in_the_flat_wire_shape() {
    let raw = r#"{"path":"app.py","kind":"file","size":6,"sha256":"ab","mode":420}"#;
    let entry: Entry = serde_json::from_str(raw).unwrap();
    assert_eq!(
        entry.kind,
        EntryKind::File {
            size: 6,
            sha256: "ab".into(),
            mode: 0o644
        }
    );
    let back: serde_json::Value = serde_json::to_value(&entry).unwrap();
    assert_eq!(
        back,
        serde_json::from_str::<serde_json::Value>(raw).unwrap()
    );
}
