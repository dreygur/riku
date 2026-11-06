use super::*;

#[test]
fn waits_for_the_socket_and_gives_up_when_it_never_appears() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("admin.sock");
    assert!(!wait_for_socket(&socket, 1), "no socket was created");

    fs::write(&socket, "").unwrap();
    assert!(wait_for_socket(&socket, 1));
}

#[test]
fn drop_in_declares_the_runtime_directory_and_is_rewritable() {
    let tmp = tempfile::tempdir().unwrap();

    write_drop_in(tmp.path()).unwrap();
    write_drop_in(tmp.path()).unwrap();

    let text =
        fs::read_to_string(tmp.path().join("caddy.service.d/riku-runtime-dir.conf")).unwrap();
    assert_eq!(
        text,
        "[Service]\nRuntimeDirectory=caddy\nRuntimeDirectoryMode=0750\n"
    );
}
