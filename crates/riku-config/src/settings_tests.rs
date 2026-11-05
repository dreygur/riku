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
fn absent_mode_means_rootless() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[engine]\ncall_timeout = \"45s\"\n").unwrap();

    let engine = load(&paths).unwrap().engine;

    assert_eq!(engine.mode, EngineMode::Rootless);
    assert_eq!(engine.host, None);
}

#[test]
fn mode_and_host_are_read() {
    let (_tmp, paths) = paths();
    std::fs::write(
        &paths.settings_file,
        "[engine]\nmode = \"rootful\"\nhost = \"unix:///run/user/1000/docker.sock\"\n",
    )
    .unwrap();

    let engine = load(&paths).unwrap().engine;

    assert_eq!(engine.mode, EngineMode::Rootful);
    assert_eq!(
        engine.host.as_deref(),
        Some("unix:///run/user/1000/docker.sock")
    );
}

#[test]
fn unknown_mode_and_bad_host_are_refused_naming_the_key() {
    let (_tmp, paths) = paths();
    for (bad, key) in [
        ("mode = \"rootles\"", "engine.mode"),
        ("mode = true", "engine.mode"),
        ("host = \"\"", "engine.host"),
        ("host = \"tcp://edge\\n:2375\"", "engine.host"),
    ] {
        std::fs::write(&paths.settings_file, format!("[engine]\n{bad}\n")).unwrap();

        let message = load(&paths).unwrap_err().to_string();

        assert!(
            message.contains(key) && message.contains("settings.toml"),
            "{bad:?} should be refused naming {key} and the file: {message}"
        );
    }
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
fn unknown_section_is_refused() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[egnine]\ncall_timeout = \"30s\"\n").unwrap();

    let message = load(&paths).unwrap_err().to_string();

    assert!(
        message.contains("egnine") && message.contains("settings.toml"),
        "error should name the section and the file: {message}"
    );
}

#[test]
fn non_string_duration_is_refused_naming_the_key() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[engine]\nbuild_timeout = 30\n").unwrap();

    let message = load(&paths).unwrap_err().to_string();

    assert!(
        message.contains("build_timeout") && message.contains("settings.toml"),
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

#[test]
fn cache_total_defaults_to_ten_and_can_be_set() {
    let (_tmp, paths) = paths();
    assert_eq!(load(&paths).unwrap().router.cache_total_gb, 10);

    std::fs::write(&paths.settings_file, "[router]\ncache_total_gb = 40\n").unwrap();

    assert_eq!(load(&paths).unwrap().router.cache_total_gb, 40);
}

#[test]
fn bad_cache_total_is_refused_naming_key_and_file() {
    let (_tmp, paths) = paths();
    for bad in ["0", "-3", "\"10\"", "1.5", "99999999999", "true"] {
        std::fs::write(
            &paths.settings_file,
            format!("[router]\ncache_total_gb = {bad}\n"),
        )
        .unwrap();

        let message = load(&paths).unwrap_err().to_string();

        assert!(
            message.contains("router.cache_total_gb") && message.contains("settings.toml"),
            "{bad:?} should be refused naming key and file: {message}"
        );
    }
}
