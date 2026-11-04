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
/// or we lack permission to write under it: both common in CI/dev
/// containers: since this is exercising kernel state, not our logic.
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

    cgroup
        .cleanup()
        .expect("cleanup should remove empty cgroup");
    assert!(!cgroup.path.exists());
}

/// A plain directory stands in for the cgroup, so this runs anywhere and
/// covers the `cgroup.procs` fallback only. The `cgroup.kill` path and the
/// removal of a really emptied cgroup need a writable cgroup v2 tree.
#[test]
fn kill_and_remove_sigkills_listed_pids_and_reports_a_cgroup_it_cannot_remove() {
    use std::os::unix::process::ExitStatusExt;

    let tmp = tempfile::tempdir().unwrap();
    let mut member = std::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .unwrap();
    std::fs::write(
        tmp.path().join("cgroup.procs"),
        format!("{}\n", member.id()),
    )
    .unwrap();
    let cgroup = WorkerCgroup {
        path: tmp.path().to_path_buf(),
    };

    let removal = cgroup.kill_and_remove();

    let status = member.wait().unwrap();
    assert_eq!(status.signal(), Some(9), "member not SIGKILLed: {status:?}");
    assert!(
        removal.is_err(),
        "a cgroup that never empties must be reported, not ignored"
    );
}

#[test]
fn test_startup_diagnostic_is_structured_and_actionable() {
    let err = anyhow::anyhow!("permission denied");
    let diagnostic = startup_diagnostic(&err);

    // Structured: every field an operator needs to triage is labeled and
    // present, not just a bare error string.
    for field in ["component", "check", "status", "detail", "impact", "remedy"] {
        assert!(
            diagnostic.contains(field),
            "diagnostic missing '{}' field:\n{}",
            field,
            diagnostic
        );
    }

    // Human-readable: names the actual path and surfaces the underlying error.
    assert!(diagnostic.contains(CGROUP_ROOT));
    assert!(diagnostic.contains("permission denied"));

    // Actionable: tells the operator what to do, not just what broke.
    assert!(
        diagnostic.to_lowercase().contains("delegat")
            || diagnostic.to_lowercase().contains("chown")
    );
}
