use super::*;

#[test]
fn test_validate_env_vars_deprecated_warnings() {
    let mut env = HashMap::new();
    env.insert("UWSGI_PROCESSES".to_string(), "4".to_string());
    env.insert("UWSGI_MAX_REQUESTS".to_string(), "1000".to_string());

    let warnings = validate_env_vars(&env);
    assert!(warnings.iter().any(|w| w.contains("UWSGI_PROCESSES")));
    assert!(warnings.iter().any(|w| w.contains("UWSGI_MAX_REQUESTS")));
}

#[test]
fn test_validate_env_vars_node_version_warning() {
    let mut env = HashMap::new();
    env.insert("RIKU_NODE_VERSION".to_string(), "invalid".to_string());

    let warnings = validate_env_vars(&env);
    assert!(warnings.iter().any(|w| w.contains("RIKU_NODE_VERSION")));
}

#[test]
fn test_validate_env_vars_clean() {
    let mut env = HashMap::new();
    env.insert("RIKU_DOMAINS".to_string(), "example.com".to_string());
    env.insert("RIKU_BIND_ADDRESS".to_string(), "127.0.0.1".to_string());

    let warnings = validate_env_vars(&env);
    assert!(warnings.is_empty());
}

#[test]
fn warns_on_unrecognized_https_only() {
    let env = HashMap::from([("RIKU_HTTPS_ONLY".to_string(), "enabled".to_string())]);

    let warnings = validate_env_vars(&env);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("RIKU_HTTPS_ONLY=enabled")),
        "{warnings:?}"
    );
}
