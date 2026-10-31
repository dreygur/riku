use super::*;
use std::time::Duration;

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    (tmp, paths)
}

#[test]
fn missing_settings_file_gives_defaults() {
    let (_tmp, paths) = paths();

    let settings = load(&paths).unwrap();

    assert_eq!(settings.engine, EngineSettings::default());
    assert_eq!(settings.engine.call_timeout, Duration::from_secs(30));
    assert_eq!(settings.engine.stop_extra, Duration::from_secs(30));
    assert_eq!(settings.engine.pull_timeout, Duration::from_secs(1800));
    assert_eq!(settings.engine.build_timeout, Duration::from_secs(1800));
}

#[test]
fn listed_keys_override_and_the_rest_keep_defaults() {
    let (_tmp, paths) = paths();
    std::fs::write(
        &paths.settings_file,
        "[engine]\ncall_timeout = \"45s\"\npull_timeout = \"2h\"\nbuild_timeout = \"10m\"\n",
    )
    .unwrap();

    let engine = load(&paths).unwrap().engine;

    assert_eq!(engine.call_timeout, Duration::from_secs(45));
    assert_eq!(engine.pull_timeout, Duration::from_secs(7200));
    assert_eq!(engine.build_timeout, Duration::from_secs(600));
    assert_eq!(engine.stop_extra, Duration::from_secs(30));
}

#[test]
fn unknown_engine_key_is_refused() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[engine]\ncall_timout = \"30s\"\n").unwrap();

    let message = load(&paths).unwrap_err().to_string();

    assert!(
        message.contains("call_timout") && message.contains("settings.toml"),
        "error should name the key and the file: {message}"
    );
}

#[test]
fn bad_duration_is_refused_naming_the_key() {
    let (_tmp, paths) = paths();
    for bad in ["30", "0s", "1d", "-5s", "s", "1.5m"] {
        std::fs::write(
            &paths.settings_file,
            format!("[engine]\nstop_extra = \"{bad}\"\n"),
        )
        .unwrap();

        let message = load(&paths).unwrap_err().to_string();

        assert!(
            message.contains("stop_extra") && message.contains("settings.toml"),
            "{bad:?} should be refused naming key and file: {message}"
        );
    }
}
