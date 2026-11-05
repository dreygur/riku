use super::*;

#[test]
fn unit_is_a_plain_service_running_the_root_binary() {
    let unit = render_unit("deploy").unwrap();

    assert!(
        unit.contains("ExecStart=/usr/local/bin/riku router-helper\n"),
        "{unit}"
    );
    assert!(unit.contains("Type=simple\n"), "{unit}");
    assert!(unit.contains("Environment=RIKU_USER=deploy\n"), "{unit}");
    assert!(!unit.contains("Socket"), "no socket activation: {unit}");
}

#[test]
fn deploy_user_name_cannot_inject_unit_lines() {
    for bad in ["", "a\nExecStartPre=/bin/sh", "a b", "a=b"] {
        assert!(render_unit(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn unit_is_written_into_the_given_directory() {
    let tmp = tempfile::tempdir().unwrap();

    write_unit(tmp.path(), "deploy").unwrap();

    assert!(tmp.path().join("riku-router-helper.service").is_file());
}
