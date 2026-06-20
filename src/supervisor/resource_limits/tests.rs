use super::*;
use std::sync::Mutex;

// `env::set_var`/`remove_var` mutate process-global state, but `cargo test`
// runs tests in parallel threads by default — without this, the *_from_env
// tests below race each other (and any test added later that touches
// RIKU_MAX_MEMORY_MB/RIKU_MAX_CPU_SECONDS) for who's set what when another
// reads it.
static ENV_VAR_LOCK: Mutex<()> = Mutex::new(());

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
fn test_from_env() {
    let _guard = ENV_VAR_LOCK.lock().unwrap();
    env::set_var("RIKU_MAX_MEMORY_MB", "256");
    env::set_var("RIKU_MAX_CPU_SECONDS", "7200");

    let limits = ResourceLimits::from_env();

    assert_eq!(limits.max_memory_bytes, Some(256 * 1024 * 1024));
    assert_eq!(limits.max_cpu_seconds, Some(7200));

    env::remove_var("RIKU_MAX_MEMORY_MB");
    env::remove_var("RIKU_MAX_CPU_SECONDS");
}

#[test]
fn test_from_env_unlimited_memory_disables_rlimit_as() {
    let _guard = ENV_VAR_LOCK.lock().unwrap();
    env::set_var("RIKU_MAX_MEMORY_MB", "unlimited");

    let limits = ResourceLimits::from_env();

    assert_eq!(limits.max_memory_bytes, None);

    env::remove_var("RIKU_MAX_MEMORY_MB");
}

#[test]
fn test_from_env_unlimited_memory_is_case_insensitive() {
    let _guard = ENV_VAR_LOCK.lock().unwrap();
    env::set_var("RIKU_MAX_MEMORY_MB", "Unlimited");

    let limits = ResourceLimits::from_env();

    assert_eq!(limits.max_memory_bytes, None);

    env::remove_var("RIKU_MAX_MEMORY_MB");
}
