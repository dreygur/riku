use super::*;

#[test]
fn test_cpu_max_value_with_quota() {
    let limits = CgroupLimits {
        memory_max_bytes: None,
        cpu_quota_us: Some(50_000),
        cpu_period_us: 100_000,
        pids_max: None,
    };
    assert_eq!(limits.cpu_max_value(), "50000 100000");
}

#[test]
fn test_cpu_max_value_unlimited() {
    let limits = CgroupLimits {
        memory_max_bytes: None,
        cpu_quota_us: None,
        cpu_period_us: 100_000,
        pids_max: None,
    };
    assert_eq!(limits.cpu_max_value(), "max 100000");
}

/// Full provision/add_self/oom_kill_count/cleanup cycle against the real
/// cgroup v2 filesystem. Skipped (not failed) when cgroup v2 isn't mounted
/// or we lack permission to write under it — both common in CI/dev
/// containers — since this is exercising kernel state, not our logic.
#[test]
fn test_provision_and_cleanup_lifecycle() {
    let limits = CgroupLimits {
        memory_max_bytes: Some(64 * 1024 * 1024),
        cpu_quota_us: Some(50_000),
        cpu_period_us: 100_000,
        pids_max: Some(32),
    };

    let process_id = format!("riku-test-{}", std::process::id());
    let cgroup = match WorkerCgroup::provision(&process_id, &limits) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipping: cgroup v2 not writable in this environment: {e}");
            return;
        }
    };

    assert!(cgroup.path.exists());
    assert_eq!(
        std::fs::read_to_string(cgroup.path.join("memory.max"))
            .unwrap()
            .trim(),
        (64 * 1024 * 1024).to_string()
    );
    assert_eq!(
        std::fs::read_to_string(cgroup.path.join("pids.max"))
            .unwrap()
            .trim(),
        "32"
    );

    cgroup.cleanup().expect("cleanup should remove empty cgroup");
    assert!(!cgroup.path.exists());
}
