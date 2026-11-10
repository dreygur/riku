use std::os::unix::fs::PermissionsExt;

use riku_util::runtime_markers::BUNDLED_RUNTIMES;

use super::*;

#[test]
fn embedded_runtimes_plus_binaries_are_the_bundled_list() {
    let mut names: Vec<&str> = EMBEDDED_RUNTIMES.to_vec();
    names.extend(super::super::install_plugins::BINARY_PLUGINS);

    assert_eq!(names, BUNDLED_RUNTIMES);
    for name in EMBEDDED_RUNTIMES {
        assert!(find(name).is_some(), "{name} is not embedded");
    }
}

#[test]
fn writing_node_needs_no_network() {
    let tmp = tempfile::tempdir().unwrap();

    write_out(find("node").unwrap(), tmp.path()).unwrap();

    let node = tmp.path().join("node");
    assert_eq!(
        std::fs::read(&node).unwrap(),
        include_bytes!("../../../../plugins/node")
    );
    assert_eq!(
        std::fs::metadata(&node).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[test]
fn riku_notify_keeps_its_bundle_layout() {
    let tmp = tempfile::tempdir().unwrap();

    write_out(find("riku-notify").unwrap(), tmp.path()).unwrap();

    let bundle = tmp.path().join("riku-notify");
    assert!(bundle.join("riku-plugin.toml").is_file());
    let entry = bundle.join("bin").join("on-event");
    assert_eq!(
        std::fs::metadata(&entry).unwrap().permissions().mode() & 0o777,
        0o755
    );
}
