use std::collections::{HashMap, HashSet};

use super::*;

fn env_with(keys: &[&str]) -> HashMap<String, String> {
    keys.iter()
        .map(|k| (k.to_string(), "x".to_string()))
        .collect()
}

#[test]
fn reports_old_with_new_name() {
    let env = env_with(&["NGINX_SERVER_NAME", "PORT"]);

    assert_eq!(
        legacy_keys(&env),
        vec![("NGINX_SERVER_NAME", Replacement::Key("RIKU_DOMAINS"))]
    );
}

#[test]
fn old_key_of_a_dropped_feature_says_remove_it() {
    let env = env_with(&["NGINX_CACHE_PATH", "DISABLE_IPV6", "ACME_EMAIL"]);

    assert_eq!(
        legacy_keys(&env),
        vec![
            ("DISABLE_IPV6", Replacement::Removed),
            ("ACME_EMAIL", Replacement::Removed),
            ("NGINX_CACHE_PATH", Replacement::Removed),
        ],
        "riku refuses or ignores the new names, so renaming would not help"
    );
}

#[test]
fn riku_names_of_never_read_keys_say_remove_them_too() {
    let env = env_with(&["RIKU_ACME_EMAIL", "RIKU_PYTHON_PACKAGE_MANAGER"]);

    let found: Vec<&str> = legacy_keys(&env).into_iter().map(|(key, _)| key).collect();

    assert_eq!(found, ["RIKU_ACME_EMAIL", "RIKU_PYTHON_PACKAGE_MANAGER"]);
    assert!(legacy_keys(&env)
        .iter()
        .all(|(_, r)| *r == Replacement::Removed));
}

#[test]
fn ignores_lookalike_keys() {
    let env = env_with(&["RIKU_DOMAINS", "NGINX_SERVER_NAME_EXTRA"]);

    assert_eq!(legacy_keys(&env), vec![]);
}

#[test]
fn table_is_unique_and_prefixed() {
    let mut seen = HashSet::new();
    for (old, replacement) in RENAMED_KEYS {
        let Replacement::Key(new) = replacement else {
            assert!(seen.insert(*old), "{old} appears twice in RENAMED_KEYS");
            continue;
        };
        assert!(new.starts_with("RIKU_"), "{old} maps to unprefixed {new}");
        assert!(seen.insert(*old), "{old} appears twice in RENAMED_KEYS");
    }
}
