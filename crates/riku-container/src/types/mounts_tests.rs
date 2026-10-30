use super::*;
use std::path::PathBuf;

fn mount(source: MountSource, read_only: bool) -> Mount {
    Mount {
        source,
        target: PathBuf::from("/data"),
        read_only,
    }
}

#[test]
fn only_read_only_mounts_are_not_storage() {
    assert!(mount(MountSource::Volume("pgdata".into()), false).is_writable());
    assert!(mount(MountSource::Anonymous, false).is_writable());
    assert!(!mount(MountSource::Bind(PathBuf::from("/srv/conf")), true).is_writable());
    assert!(!mount(MountSource::Volume("pgdata".into()), true).is_writable());
}
