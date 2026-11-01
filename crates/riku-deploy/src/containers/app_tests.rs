use std::fs;

use super::*;
use crate::config::RikuPaths;

#[test]
fn read_scaling_rejects_empty_service_name() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(env_dir.join("SCALING"), "=2\n").unwrap();

    let err = read_scaling("myapp", &paths).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("=2"),
        "error does not name the line: {message}"
    );
}
