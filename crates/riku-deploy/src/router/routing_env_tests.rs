use std::collections::HashMap;

use super::*;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::router::routing_state::RoutingState;

const ROUTING: &str =
    "[routing]\nfallback_domain = \"{app}.{ip}.example\"\nserver_ip = \"203.0.113.10\"\n";

fn paths_with(settings: Option<&str>) -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    if let Some(text) = settings {
        std::fs::write(&paths.settings_file, text).unwrap();
    }
    (tmp, paths)
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn fallback_overlay_sets_domains_and_drops_https_only() {
    let (_tmp, paths) = paths_with(Some(ROUTING));

    let (routed_env, routing) = routing_env(
        "myapp",
        &env(&[("RIKU_HTTPS_ONLY", "true")]),
        &paths,
        &Router::Nginx,
        Record::Save,
    )
    .unwrap();

    assert!(routing.fallback);
    assert_eq!(
        routed_env.get("RIKU_DOMAINS").map(String::as_str),
        Some("myapp.203.0.113.10.example")
    );
    assert!(
        !routed_env.contains_key("RIKU_HTTPS_ONLY"),
        "https_only applies to the app's own domains only"
    );
}

#[test]
fn no_routing_section_keeps_todays_env() {
    let (_tmp, paths) = paths_with(Some("[router]\nname = \"nginx\"\n"));
    let original = env(&[("RIKU_HTTPS_ONLY", "true")]);

    let (routed_env, routing) =
        routing_env("myapp", &original, &paths, &Router::Nginx, Record::Save).unwrap();

    assert!(routing.legacy);
    assert_eq!(routed_env, original);
}

#[test]
fn port_mode_overlay_sets_the_picked_port() {
    let (_tmp, paths) = paths_with(Some("[routing]\nport_range = \"9100-9110\"\n"));

    let (routed_env, routing) =
        routing_env("myapp", &env(&[]), &paths, &Router::Nginx, Record::Save).unwrap();

    assert_eq!(routing.port, Some(9100));
    assert_eq!(
        routed_env.get("RIKU_ROUTING_PORT").map(String::as_str),
        Some("9100")
    );
}

#[test]
fn save_records_the_entry_and_peek_does_not() {
    let (_tmp, paths) = paths_with(Some(ROUTING));

    routing_env("peeked", &env(&[]), &paths, &Router::Nginx, Record::Peek).unwrap();
    routing_env("saved", &env(&[]), &paths, &Router::Nginx, Record::Save).unwrap();
    let state = RoutingState::load(&paths).unwrap();

    assert!(state.entry("peeked").is_none());
    assert!(state.entry("saved").is_some());
}
