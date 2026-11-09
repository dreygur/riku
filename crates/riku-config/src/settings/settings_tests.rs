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
fn router_and_engine_program_are_read() {
    let (_tmp, paths) = paths();
    std::fs::write(
        &paths.settings_file,
        "[router]\nname = \"caddy\"\n[engine]\nprogram = \"/usr/bin/podman\"\n",
    )
    .unwrap();

    let settings = load(&paths).unwrap();

    assert_eq!(settings.router.as_deref(), Some("caddy"));
    assert_eq!(settings.engine.program.as_deref(), Some("/usr/bin/podman"));
}

#[test]
fn unknown_router_key_is_refused() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[router]\nnmae = \"caddy\"\n").unwrap();

    let message = load(&paths).unwrap_err().to_string();

    assert!(
        message.contains("router.nmae") && message.contains("settings.toml"),
        "error should name the key and the file: {message}"
    );
}

#[test]
fn save_key_keeps_other_sections_and_removes_a_key() {
    let (_tmp, paths) = paths();
    std::fs::write(&paths.settings_file, "[engine]\nstop_extra = \"1m\"\n").unwrap();

    save_key(&paths, "router", "name", Some("caddy")).unwrap();
    let settings = load(&paths).unwrap();
    assert_eq!(settings.router.as_deref(), Some("caddy"));
    assert_eq!(settings.engine.stop_extra, Duration::from_secs(60));

    save_key(&paths, "router", "name", None).unwrap();
    assert_eq!(load(&paths).unwrap().router, None);
}

#[test]
fn save_key_refuses_a_value_the_file_would_reject() {
    let (_tmp, paths) = paths();

    let result = save_key(&paths, "router", "nmae", Some("caddy"));

    assert!(result.is_err());
    assert!(
        !paths.settings_file.exists(),
        "a refused save must not write the file"
    );
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
