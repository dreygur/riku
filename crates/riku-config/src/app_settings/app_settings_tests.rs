use std::collections::HashMap;
use std::path::Path;

use super::*;

const FILE: &str = "/srv/myapp/riku.toml";

fn repo(raw: &str) -> anyhow::Result<AppSettings> {
    AppSettings::parse(raw, Path::new(FILE), Source::Repo)
}

fn server(raw: &str) -> anyhow::Result<AppSettings> {
    AppSettings::parse(raw, Path::new(FILE), Source::Server)
}

fn err(result: anyhow::Result<AppSettings>) -> String {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn maps_domains_and_https_to_env() {
    let settings =
        repo("[routing]\ndomains = [\"a.example\", \"b.example\"]\nhttps_only = true\n").unwrap();

    let env = settings.env();

    assert_eq!(
        env.get("RIKU_DOMAINS").map(String::as_str),
        Some("a.example,b.example")
    );
    assert_eq!(env.get("RIKU_HTTPS_ONLY").map(String::as_str), Some("true"));
}

#[test]
fn false_flag_is_omitted() {
    let settings = repo("[routing]\nstatic = false\n").unwrap();

    let env = settings.env();

    assert!(
        !env.contains_key("RIKU_NGINX_STATIC"),
        "a false flag must not be present"
    );
}

#[test]
fn refuses_unknown_key_with_line_and_without_value() {
    let message = err(repo("[routing]\n\ndomain = \"secret-value.example\"\n"));

    assert!(message.contains(FILE), "names the file: {message}");
    assert!(
        message.contains("routing.domain"),
        "names the key: {message}"
    );
    assert!(message.contains(":3:"), "names the line: {message}");
    assert!(
        !message.contains("secret-value"),
        "must not echo the value: {message}"
    );
}

#[test]
fn maps_routing_mode_and_port_to_env() {
    let env = repo("[routing]\nmode = \"port\"\nport = 8181\n")
        .unwrap()
        .env();

    assert_eq!(
        env.get("RIKU_ROUTING_MODE").map(String::as_str),
        Some("port")
    );
    assert_eq!(
        env.get("RIKU_ROUTING_PORT").map(String::as_str),
        Some("8181")
    );
}

#[test]
fn mode_outside_three_words_is_refused_with_line() {
    let message = err(repo("[routing]\n\nmode = \"tunnel\"\n"));

    assert!(message.contains("routing.mode"), "{message}");
    assert!(message.contains(":3:"), "names the line: {message}");
}

#[test]
fn port_above_65535_is_refused_with_line() {
    let message = err(repo("[routing]\nport = 70000\n"));

    assert!(message.contains("routing.port"), "{message}");
    assert!(message.contains(":2:"), "names the line: {message}");
}

#[test]
fn refuses_wrong_type() {
    let message = err(repo("[routing]\nhttps_only = \"yes\"\n"));

    assert!(message.contains("routing.https_only"), "{message}");
}

#[test]
fn refuses_server_only_keys_in_repo() {
    for raw in [
        "[secrets]\nghcr_token = \"abc\"\n",
        "[limits]\nmax_memory_mb = 4096\n",
        "[routing]\nbind_address = \"0.0.0.0\"\n",
        "[limits]\nisolation_root = \"/srv/jail\"\n",
    ] {
        let message = err(repo(raw));
        assert!(message.contains("riku settings set"), "{raw}: {message}");
    }
}

#[test]
fn isolation_root_must_be_a_plain_absolute_path() {
    for root in ["srv/jail", "/srv/../etc", "/srv/ja il", "/srv/$HOME", "/"] {
        let raw = format!("[limits]\nisolation_root = \"{root}\"\n");
        let message = err(server(&raw));
        assert!(
            message.contains("limits.isolation_root"),
            "{root}: {message}"
        );
    }

    let env = server("[limits]\nisolation_root = \"/srv/jail-1/app.d\"\n")
        .unwrap()
        .env();
    assert_eq!(
        env.get("RIKU_ISOLATION_ROOT").map(String::as_str),
        Some("/srv/jail-1/app.d")
    );
}

#[test]
fn allows_server_only_keys_in_server_file() {
    let settings =
        server("[secrets]\nghcr_token = \"abc\"\n[limits]\nmax_memory_mb = 4096\n").unwrap();

    let env = settings.env();

    assert_eq!(env.get("RIKU_GHCR_TOKEN").map(String::as_str), Some("abc"));
    assert_eq!(
        env.get("RIKU_MAX_MEMORY_MB").map(String::as_str),
        Some("4096")
    );
}

#[test]
fn refuses_escaping_values() {
    for raw in [
        "[routing]\ndocument_root = \"../../etc\"\n",
        "[routing]\ndocument_root = \"/etc\"\n",
        "[routing]\ndomains = [\"a.example;evil\"]\n",
        "[routing]\ndomains = [\"a.example\\nevil\"]\n",
    ] {
        assert!(repo(raw).is_err(), "should refuse: {raw}");
    }
}

#[test]
fn runtime_version_uses_runtime_name() {
    let settings = repo("[runtime]\nname = \"rust-lang\"\nversion = \"1.80\"\n").unwrap();

    let env = settings.env();

    assert_eq!(
        env.get("RIKU_RUNTIME").map(String::as_str),
        Some("rust-lang")
    );
    assert_eq!(
        env.get("RIKU_RUST_LANG_VERSION").map(String::as_str),
        Some("1.80")
    );
}

#[test]
fn runtime_version_without_name_is_refused() {
    let message = err(repo("[runtime]\nversion = \"22\"\n"));

    assert!(message.contains("runtime.version"), "{message}");
}

#[test]
fn cron_section_becomes_cron_kind() {
    let settings =
        repo("[processes]\nweb = \"node server.js\"\n[cron]\nbackup = \"0 2 * * * ./backup.sh\"\n")
            .unwrap();

    let processes = settings.processes().unwrap();

    assert_eq!(
        processes.get("web").map(String::as_str),
        Some("node server.js")
    );
    assert_eq!(
        processes.get("cron-backup").map(String::as_str),
        Some("0 2 * * * ./backup.sh")
    );
}

#[test]
fn refuses_bad_cron_and_cron_key_in_processes() {
    assert!(repo("[cron]\nbackup = \"every night ./backup.sh\"\n").is_err());
    assert!(repo("[processes]\ncronjob = \"0 2 * * * ./x\"\n").is_err());
}

#[test]
fn refuses_processes_in_server_file() {
    assert!(server("[processes]\nweb = \"node server.js\"\n").is_err());
}

#[test]
fn refuses_web_and_wsgi_together() {
    assert!(repo("[processes]\nweb = \"a\"\nwsgi = \"b\"\n").is_err());
}

#[test]
fn no_process_sections_means_none() {
    assert!(repo("[routing]\nhttps_only = true\n")
        .unwrap()
        .processes()
        .is_none());
}

#[test]
fn from_env_round_trips_table_keys_only() {
    let mut env = HashMap::new();
    env.insert(
        "RIKU_DOMAINS".to_string(),
        "a.example,b.example".to_string(),
    );
    env.insert("RIKU_WORKER_TIMEOUT".to_string(), "60".to_string());
    env.insert("RIKU_RUNTIME".to_string(), "node".to_string());
    env.insert("RIKU_NODE_VERSION".to_string(), "22".to_string());
    env.insert("DATABASE_URL".to_string(), "postgres://x".to_string());

    let (settings, moved) = AppSettings::from_env(&env);
    let back = server(&settings.to_toml().unwrap()).unwrap().env();

    assert_eq!(back.get("RIKU_DOMAINS"), env.get("RIKU_DOMAINS"));
    assert_eq!(
        back.get("RIKU_WORKER_TIMEOUT"),
        env.get("RIKU_WORKER_TIMEOUT")
    );
    assert_eq!(back.get("RIKU_NODE_VERSION"), env.get("RIKU_NODE_VERSION"));
    assert!(!back.contains_key("DATABASE_URL"));
    assert!(!moved.contains(&"DATABASE_URL".to_string()));
    assert_eq!(moved.len(), 4, "moved: {moved:?}");
}

#[test]
fn set_get_unset_on_server_settings() {
    let mut settings = server("").unwrap();

    settings
        .set("routing.domains", "a.example, b.example")
        .unwrap();
    assert_eq!(
        settings.get("routing.domains").as_deref(),
        Some("a.example,b.example")
    );
    assert!(settings.set("routing.domain", "x").is_err());
    assert!(settings.set("workers.timeout", "soon").is_err());

    settings.unset("routing.domains");
    assert_eq!(settings.get("routing.domains"), None);
}
