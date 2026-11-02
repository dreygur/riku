use std::fs;
use std::time::Duration;

use super::*;
use crate::config::RikuPaths;
use crate::mock_engine::mock_engine;

#[test]
fn open_engine_applies_server_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let (_engine, _log) = mock_engine(tmp.path(), "");
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        &paths.container_engine_file,
        tmp.path().join("engine").display().to_string(),
    )
    .unwrap();
    fs::write(&paths.settings_file, "[engine]\ncall_timeout = \"5s\"\n").unwrap();

    assert_eq!(
        open_engine(&paths).unwrap().settings().call_timeout,
        Duration::from_secs(5),
        "open_engine did not apply call_timeout from settings.toml"
    );
}
