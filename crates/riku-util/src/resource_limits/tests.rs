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

#[test]
fn memory_stays_unset_when_not_configured() {
    let limits = ResourceLimits::from_lookup(|_| None);

    assert_eq!(
        limits.max_memory_bytes, None,
        "RLIMIT_AS is opt-in: an unset RIKU_MAX_MEMORY_MB must leave it uncapped"
    );
}
