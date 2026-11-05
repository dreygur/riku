use super::*;

#[test]
fn host_file_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(HOST_FILE_NAME);
    for init in [InitSystem::Systemd, InitSystem::OpenRc, InitSystem::Other] {
        fs::write(&path, HostFacts { init }.to_toml()).unwrap();
        let read = read_host(&path).unwrap();
        assert_eq!(read, Some(HostFacts { init }));
    }
}

#[test]
fn missing_file_is_not_a_default() {
    let dir = tempfile::tempdir().unwrap();
    let read = read_host(&dir.path().join(HOST_FILE_NAME)).unwrap();
    assert_eq!(read, None);
}

#[test]
fn damaged_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(HOST_FILE_NAME);
    fs::write(&path, "init = \"runit\"\n").unwrap();
    let err = read_host(&path).unwrap_err().to_string();
    assert!(err.contains("init must be"), "got: {err}");
}
