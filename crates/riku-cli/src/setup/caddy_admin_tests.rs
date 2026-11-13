use super::*;

#[test]
fn waits_for_the_socket_and_gives_up_when_it_never_appears() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("admin.sock");
    assert!(!wait_for_socket(&socket, 1), "no socket was created");

    fs::write(&socket, "").unwrap();
    assert!(wait_for_socket(&socket, 1));
}

/// A second `riku init` with nothing to change must not bounce Caddy, which
/// would drop every site for a moment.
#[test]
fn caddy_restarts_only_to_move_onto_the_admin_socket() {
    let settled = CaddyState {
        drop_in_changed: false,
        caddyfile_changed: false,
        on_socket: true,
    };
    assert_eq!(caddy_action(settled), CaddyAction::Nothing);
    let edited = CaddyState {
        caddyfile_changed: true,
        ..settled
    };
    assert_eq!(caddy_action(edited), CaddyAction::Reload);
    let first_run = CaddyState {
        on_socket: false,
        ..settled
    };
    assert_eq!(caddy_action(first_run), CaddyAction::Restart);
    let new_drop_in = CaddyState {
        drop_in_changed: true,
        ..settled
    };
    assert_eq!(caddy_action(new_drop_in), CaddyAction::Restart);
}

#[test]
fn drop_in_declares_the_runtime_directory_and_is_rewritable() {
    let tmp = tempfile::tempdir().unwrap();

    assert!(write_drop_in(tmp.path()).unwrap(), "first write changes it");
    assert!(
        !write_drop_in(tmp.path()).unwrap(),
        "same text is no change"
    );

    let text =
        fs::read_to_string(tmp.path().join("caddy.service.d/riku-runtime-dir.conf")).unwrap();
    assert_eq!(
        text,
        "[Service]\nRuntimeDirectory=caddy\nRuntimeDirectoryMode=0750\n"
    );
}
