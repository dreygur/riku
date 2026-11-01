use std::fs;

use super::*;

#[test]
fn agent_restart_with_service_on_procfile_app_fails_not_container_app() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let resp = cmd_agent_restart(&paths, "shop", Some("web"));
    assert!(
        !resp.success,
        "expected restart to fail for Procfile app with service"
    );

    let err = match resp.error {
        Some(err) => err,
        None => panic!("expected error in response, got none"),
    };
    assert!(
        err.message.contains("not a container app"),
        "expected error message to say not a container app, got: {}",
        err.message
    );
}
