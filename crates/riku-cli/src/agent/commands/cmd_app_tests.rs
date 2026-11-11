use std::fs;

use super::*;

/// `~/.riku/apps/..` exists, so a raw `exists()` check let `..` through to
/// the human deploy path, which exits the process instead of answering.
#[test]
fn deploy_refuses_a_path_as_app_name_with_json() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();

    for name in ["..", "."] {
        let resp = cmd_agent_deploy(&paths, name);
        let code = resp.error.map(|e| e.code);
        assert_eq!(code.as_deref(), Some("APP_NOT_FOUND"), "app name {name:?}");
    }
}
