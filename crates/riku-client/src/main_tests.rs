use crate::local::check_supported;

#[test]
fn refuses_container_and_server_only_commands_but_not_dashboard() {
    let message = |command| check_supported(command).unwrap_err().to_string();
    assert_eq!(
        message("init"),
        "'init' is not part of rk; run it with riku on the server"
    );
    assert_eq!(
        message("container"),
        "'container' is not part of rk; run it with the full riku binary"
    );
    assert!(check_supported("dashboard").is_ok());
    assert!(check_supported("apps").is_ok());
}
