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
fn logs_returns_the_newest_hundred_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.log_root.join("shop")).unwrap();
    let body: String = (1..=150).map(|n| format!("line {n}\n")).collect();
    fs::write(paths.log_root.join("shop").join("web.1.log"), body).unwrap();

    let resp = cmd_agent_logs(&paths, "shop", "*");

    let data = resp.data.expect("logs returned no data");
    let lines = data["lines"].as_array().expect("lines is not an array");
    assert_eq!(lines.len(), 100);
    assert_eq!(lines.first().unwrap(), "line 51", "oldest kept line");
    assert_eq!(lines.last().unwrap(), "line 150", "newest line");
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
fn agent_restart_reads_the_second_argument_of_a_procfile_app_as_a_process_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    fs::write(
        paths.workers_enabled.join("shop@web.1.toml"),
        "[worker]\napp = \"shop\"\nkind = \"web\"\n",
    )
    .unwrap();

    let resp = cmd_agent_restart(&paths, "shop", Some("db"));

    let err = match resp.error {
        Some(err) => err,
        None => panic!("restart of a kind shop does not run must fail"),
    };
    assert!(
        err.message.contains("runs no process 'db'") && err.message.contains("web"),
        "the error must name the unknown kind and the app's kinds, got: {}",
        err.message
    );
}
