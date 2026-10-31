use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn reload_nginx_error_is_returned() {
    let tmp = tempfile::tempdir().unwrap();
    let mock_nginx = tmp.path().join("mock_nginx");
    std::fs::write(
        &mock_nginx,
        "#!/bin/sh\necho 'nginx: [emerg] bind() failed' >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&mock_nginx, std::fs::Permissions::from_mode(0o755)).unwrap();

    let err = reload_nginx_with(&mock_nginx).unwrap_err().to_string();

    assert!(err.contains("bind() failed"), "{err}");
}
