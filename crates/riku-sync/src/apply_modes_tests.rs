use crate::testkit::{names_in, put, sync_from, SETTLED};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn mode_of(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn directory_modes_survive_a_sync() {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "private/key.txt", "k");
    put(src.path(), "shared/doc.txt", "d");
    set_mode(&src.path().join("private"), 0o700);
    set_mode(&src.path().join("shared"), 0o750);
    set_mode(src.path(), 0o750);
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");

    sync_from(src.path(), &live);

    assert_eq!(mode_of(&live.join("private")), 0o700);
    assert_eq!(mode_of(&live.join("shared")), 0o750);
    assert_eq!(mode_of(&live), 0o750, "the live root keeps the source mode");
}

#[test]
fn read_only_directory_can_be_synced_again() {
    // The old tree is deleted after the swap; a 0555 directory in it must not
    // block that, or the next sync fails on the leftover.
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "ro/data.txt", "v1");
    set_mode(&src.path().join("ro"), 0o555);
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");

    sync_from(src.path(), &live);
    put(src.path(), "top.txt", "new");
    sync_from(src.path(), &live);

    assert_eq!(mode_of(&live.join("ro")), 0o555);
    assert_eq!(fs::read_to_string(live.join("top.txt")).unwrap(), "new");
    assert_eq!(names_in(parent.path()), SETTLED, "old tree left behind");

    // Let the temp dirs remove themselves.
    set_mode(&live.join("ro"), 0o755);
    set_mode(&src.path().join("ro"), 0o755);
}
