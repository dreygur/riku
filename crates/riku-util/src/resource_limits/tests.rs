use std::collections::HashMap;

use super::*;

#[test]
fn test_default_limits() {
    let limits = ResourceLimits::default();

    assert_eq!(limits.max_memory_bytes, Some(512 * 1024 * 1024));
    assert_eq!(limits.max_cpu_seconds, Some(3600));
    assert_eq!(limits.max_open_files, Some(1024));
    assert_eq!(limits.max_processes, None);
    assert_eq!(limits.max_core_file_bytes, Some(0));
}

#[test]
fn test_summary() {
    let limits = ResourceLimits::default();
    let summary = limits.summary();

    assert!(summary.contains("mem=512MB"));
    assert!(summary.contains("cpu=3600s"));
    assert!(summary.contains("files=1024"));
    assert!(!summary.contains("procs="));
}

#[test]
fn test_summary_with_max_processes_opted_in() {
    let limits = ResourceLimits {
        max_processes: Some(64),
        ..ResourceLimits::default()
    };
    assert!(limits.summary().contains("procs=64"));
}

#[test]
fn configured_memory_and_cpu_override_the_defaults() {
    let limits = ResourceLimits::from_lookup(|key| match key {
        "RIKU_MAX_MEMORY_MB" => Some(256),
        "RIKU_MAX_CPU_SECONDS" => Some(7200),
        _ => None,
    });

    assert_eq!(limits.max_memory_bytes, Some(256 * 1024 * 1024));
    assert_eq!(limits.max_cpu_seconds, Some(7200));
}

fn server_limits() -> ResourceLimits {
    ResourceLimits::from_lookup(|key| match key {
        "RIKU_MAX_OPEN_FILES" => Some(2048),
        "RIKU_MAX_CPU_SECONDS" => Some(600),
        _ => None,
    })
}

fn app_env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
fn app_limits_override_the_server_limits_they_name() {
    let env = app_env(&[
        ("RIKU_MAX_OPEN_FILES", "64"),
        ("RIKU_MAX_MEMORY_MB", "256"),
        ("RIKU_MAX_FILE_SIZE_MB", "10"),
    ]);

    let limits = server_limits().with_app_env(&env);

    assert_eq!(limits.max_open_files, Some(64));
    assert_eq!(limits.max_memory_bytes, Some(256 * 1024 * 1024));
    assert_eq!(limits.max_file_size_bytes, Some(10 * 1024 * 1024));
    assert_eq!(
        limits.max_cpu_seconds,
        Some(600),
        "a limit the app does not set must keep the server value"
    );
}

#[test]
fn app_env_without_limit_keys_keeps_the_server_limits() {
    let env = app_env(&[
        ("DATABASE_URL", "postgres://db"),
        ("RIKU_MAX_CPU_SECONDS", "lots"),
    ]);

    let limits = server_limits().with_app_env(&env);

    assert_eq!(limits, server_limits());
}

#[test]
fn megabyte_values_too_large_for_bytes_keep_the_base_limit() {
    let huge = u64::MAX.to_string();
    let env = app_env(&[
        ("RIKU_MAX_MEMORY_MB", huge.as_str()),
        ("RIKU_MAX_FILE_SIZE_MB", huge.as_str()),
    ]);

    let limits = server_limits().with_app_env(&env);

    assert_eq!(
        (limits.max_memory_bytes, limits.max_file_size_bytes),
        (
            server_limits().max_memory_bytes,
            server_limits().max_file_size_bytes
        ),
        "a value whose byte count overflows u64 must be ignored like an unparsable one"
    );
}

#[test]
fn memory_stays_unset_when_not_configured() {
    let limits = ResourceLimits::from_lookup(|_| None);

    assert_eq!(
        limits.max_memory_bytes, None,
        "RLIMIT_AS is opt-in: an unset RIKU_MAX_MEMORY_MB must leave it uncapped"
    );
}
