use super::*;

/// A running supervisor holds the installed binary open, and Linux refuses
/// writes into it (ETXTBSY). The old file must be left untouched: a hard
/// link to it still reads the old bytes after the replace.
#[test]
fn replaces_the_binary_without_writing_into_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("new-riku");
    let target = dir.path().join("riku");
    let running = dir.path().join("running-riku");
    fs::write(&source, "new").unwrap();
    fs::write(&target, "old").unwrap();
    fs::hard_link(&target, &running).unwrap();

    replace_binary(&source, &target).unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    assert_eq!(fs::read_to_string(&running).unwrap(), "old");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "installed mode is {mode:o}");
}
