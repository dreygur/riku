use std::fs;

use super::*;

#[test]
fn missing_app_answers_json_instead_of_exiting() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.app_root).unwrap();

    let resp = cmd_agent_stop(&paths, tmp.path(), "ghost");

    let code = resp.error.map(|e| e.code);
    assert_eq!(code.as_deref(), Some("APP_NOT_FOUND"));
}

#[test]
fn agent_run_and_config_set_say_they_are_not_implemented() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let run = cmd_agent_run(&paths, "shop", &["ls".to_string()]);
    let set = super::super::cmd_config::cmd_agent_config_set(&paths, "shop", &["A=1".to_string()]);

    for (name, resp) in [("run", run), ("config:set", set)] {
        assert!(
            !resp.success,
            "{name} reported success without doing anything"
        );
        let code = resp.error.map(|e| e.code);
        assert_eq!(code.as_deref(), Some("NOT_IMPLEMENTED"), "{name}");
    }
}

#[test]
fn stop_confirm_refuses_a_token_that_was_never_issued() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let resp = cmd_agent_stop_confirm(&paths, tmp.path(), "shop", "made-up");
    let code = resp.error.map(|e| e.code);
    assert_eq!(
        code.as_deref(),
        Some("INVALID_TOKEN"),
        "stop ran without a valid token"
    );
}

#[test]
fn stop_request_then_confirm_with_its_token_reaches_stop() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    let token = match cmd_agent_stop(&paths, tmp.path(), "shop").confirm_token {
        Some(token) => token,
        None => panic!("stop request returned no token"),
    };
    let resp = cmd_agent_stop_confirm(&paths, tmp.path(), "shop", &token);
    let code = resp.error.map(|e| e.code);
    assert_ne!(
        code.as_deref(),
        Some("INVALID_TOKEN"),
        "the issued token was refused"
    );
}

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
