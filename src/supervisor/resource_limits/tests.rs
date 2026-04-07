use super::*;

#[test]
fn test_default_limits() {
    let limits = ResourceLimits::default();

    assert_eq!(limits.max_memory_bytes, Some(512 * 1024 * 1024));
    assert_eq!(limits.max_cpu_seconds, Some(3600));
    assert_eq!(limits.max_open_files, Some(1024));
    assert_eq!(limits.max_processes, Some(64));
    assert_eq!(limits.max_core_file_bytes, Some(0));
}

#[test]
fn test_summary() {
    let limits = ResourceLimits::default();
    let summary = limits.summary();

    assert!(summary.contains("mem=512MB"));
    assert!(summary.contains("cpu=3600s"));
    assert!(summary.contains("files=1024"));
    assert!(summary.contains("procs=64"));
}

#[test]
fn test_from_env() {
    env::set_var("RIKU_MAX_MEMORY_MB", "256");
    env::set_var("RIKU_MAX_CPU_SECONDS", "7200");

    let limits = ResourceLimits::from_env();

    assert_eq!(limits.max_memory_bytes, Some(256 * 1024 * 1024));
    assert_eq!(limits.max_cpu_seconds, Some(7200));

    env::remove_var("RIKU_MAX_MEMORY_MB");
    env::remove_var("RIKU_MAX_CPU_SECONDS");
}
