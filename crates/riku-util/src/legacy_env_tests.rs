use std::collections::{HashMap, HashSet};

use super::*;

fn env_with(keys: &[&str]) -> HashMap<String, String> {
    keys.iter()
        .map(|k| (k.to_string(), "x".to_string()))
        .collect()
}

#[test]
fn reports_old_keys_with_their_new_name() {
    let env = env_with(&["NGINX_SERVER_NAME", "PORT"]);

    assert_eq!(
        legacy_keys(&env),
        vec![("NGINX_SERVER_NAME", "RIKU_DOMAINS")]
    );
}

#[test]
fn ignores_new_and_lookalike_keys() {
    let env = env_with(&["RIKU_DOMAINS", "NGINX_SERVER_NAME_EXTRA"]);

    assert_eq!(legacy_keys(&env), vec![]);
}

#[test]
fn table_has_no_duplicate_or_unprefixed_targets() {
    let mut seen = HashSet::new();
    for (old, new) in RENAMED_KEYS {
        assert!(new.starts_with("RIKU_"), "{old} maps to unprefixed {new}");
        assert!(seen.insert(*old), "{old} appears twice in RENAMED_KEYS");
    }
}
