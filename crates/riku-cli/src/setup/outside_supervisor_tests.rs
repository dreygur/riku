use std::fs;
use std::path::Path;

use super::*;

/// `<proc>/<pid>/exe` pointing at `binary`, as the kernel shows it.
fn running_from(proc_root: &Path, pid: u32, binary: &Path) {
    let dir = proc_root.join(pid.to_string());
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(binary, dir.join("exe")).unwrap();
}

/// A supervisor a deploy started with nohup keeps the pid file lock, so the
/// unit's own could never start while it runs the old binary.
#[test]
fn only_an_old_supervisor_outside_the_unit_is_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old-riku");
    fs::write(&old, "riku 0.1.3").unwrap();
    let installed = tmp.path().join("riku");
    fs::write(&installed, "riku 0.1.4").unwrap();
    let proc_root = tmp.path().join("proc");
    running_from(&proc_root, 51, &old);
    running_from(&proc_root, 52, &installed);

    assert_eq!(to_replace(None, Some(51), &proc_root, &installed), Some(51));
    assert_eq!(
        to_replace(None, Some(52), &proc_root, &installed),
        None,
        "an outside supervisor on the installed binary is left alone"
    );
    assert_eq!(
        to_replace(Some(51), Some(51), &proc_root, &installed),
        None,
        "the unit's own supervisor is root's to restart"
    );
    assert_eq!(to_replace(None, None, &proc_root, &installed), None);
}
