use std::fs;

use super::*;

fn app(tmp: &Path) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    paths
}

fn write(path: PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

#[test]
fn repo_settings_reach_env() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.app_settings_repo("shop"),
        "[routing]\ndomains = [\"shop.example\"]\n",
    );

    let loaded = load_env(&paths, "shop").unwrap();

    assert_eq!(
        loaded.env.get("RIKU_DOMAINS").map(String::as_str),
        Some("shop.example")
    );
}

#[test]
fn a_setting_key_in_env_is_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.app_settings_repo("shop"),
        "[routing]\ndomains = [\"repo.example\"]\n",
    );
    write(
        paths.env_root.join("shop").join("ENV"),
        "RIKU_DOMAINS=env.example\nPORT=5000\n",
    );

    let loaded = load_env(&paths, "shop").unwrap();

    assert_eq!(
        loaded.env.get("RIKU_DOMAINS").map(String::as_str),
        Some("repo.example"),
        "riku.toml decides the setting, not ENV"
    );
    assert_eq!(loaded.env.get("PORT").map(String::as_str), Some("5000"));
}

#[test]
fn server_override_beats_env_file() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.env_root.join("shop").join("ENV"),
        "RIKU_DOMAINS=env.example\n",
    );
    write(
        paths.app_settings_override("shop"),
        "[routing]\ndomains = [\"server.example\"]\n",
    );

    let loaded = load_env(&paths, "shop").unwrap();

    assert_eq!(
        loaded.env.get("RIKU_DOMAINS").map(String::as_str),
        Some("server.example")
    );
}

#[test]
fn server_override_false_turns_a_flag_off() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.app_settings_repo("shop"),
        "[routing]\nhttps_only = true\nstatic = true\n",
    );
    write(
        paths.app_settings_override("shop"),
        "[routing]\nhttps_only = false\nstatic = false\n",
    );

    let env = load_env(&paths, "shop").unwrap().env;

    assert!(!env.contains_key("RIKU_HTTPS_ONLY"), "env: {env:?}");
    assert!(!env.contains_key("RIKU_NGINX_STATIC"), "env: {env:?}");
}

#[test]
fn repeated_procfile_cron_lines_are_all_jobs() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.app_root.join("shop").join("Procfile"),
        "web: node server.js\ncron: 0 1 * * * ./a\ncron: 0 2 * * * ./b\n",
    );

    let entries = load_cron_entries(&paths, "shop").unwrap();

    let commands: Vec<&str> = entries.iter().map(|(_, cmd)| cmd.as_str()).collect();
    assert_eq!(commands, vec!["0 1 * * * ./a", "0 2 * * * ./b"]);
}

#[test]
fn invalid_repo_settings_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.app_settings_repo("shop"),
        "[routing]\ndomain = \"x\"\n",
    );

    assert!(load_env(&paths, "shop").is_err());
}

#[test]
fn riku_toml_processes_replace_procfile() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.app_settings_repo("shop"),
        "[processes]\nweb = \"node server.js\"\n",
    );

    let processes = load_processes(&paths, "shop").unwrap().unwrap();

    assert_eq!(
        processes.get("web").map(String::as_str),
        Some("node server.js")
    );
}

#[test]
fn both_process_sources_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    write(
        paths.app_settings_repo("shop"),
        "[processes]\nweb = \"node server.js\"\n",
    );
    write(
        paths.app_root.join("shop").join("Procfile"),
        "web: node other.js\n",
    );

    let message = load_processes(&paths, "shop").unwrap_err().to_string();

    assert!(message.contains("keep one"), "{message}");
}

#[test]
fn no_riku_toml_matches_env_and_procfile() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app(tmp.path());
    let env_file = paths.env_root.join("shop").join("ENV");
    let procfile = paths.app_root.join("shop").join("Procfile");
    write(env_file.clone(), "PORT=5000\nDATABASE_URL=postgres://db\n");
    write(
        procfile.clone(),
        "web: node server.js\nworker: node jobs.js\n",
    );

    let mut expected_env = HashMap::new();
    let expected_env = riku_util::parse_settings(&env_file, &mut expected_env).unwrap();
    let expected_processes = riku_util::parse_procfile(&procfile).unwrap();

    assert_eq!(load_env(&paths, "shop").unwrap().env, expected_env);
    assert_eq!(load_processes(&paths, "shop").unwrap(), expected_processes);
}
