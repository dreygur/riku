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

/// A fake cgroup tree in a tempdir: this checks placement and the values
/// written, not that the kernel enforces them.
#[test]
fn provision_places_the_worker_under_apps_and_writes_its_limits() {
    let own = tempfile::tempdir().unwrap();
    let proc_text = "0::/riku.service\n";
    let tree = CgroupTree::locate(own.path(), proc_text).unwrap();
    std::fs::create_dir_all(tree.apps_dir()).unwrap();
    let limits = CgroupLimits {
        memory_max_bytes: Some(64 * 1024 * 1024),
        cpu_quota_us: Some(50_000),
        cpu_period_us: 100_000,
        pids_max: Some(32),
    };

    let worker = WorkerName::new("shop", "web", 1);
    let cgroup = WorkerCgroup::provision(&tree, &worker, &limits).unwrap();

    assert_eq!(cgroup.path, own.path().join("riku.service/apps/shop@web.1"));
    for (node, want) in [
        ("memory.max", "67108864"),
        ("cpu.max", "50000 100000"),
        ("pids.max", "32"),
    ] {
        let got = std::fs::read_to_string(cgroup.path.join(node)).unwrap();
        assert_eq!(got, want, "{node}");
    }
}

#[test]
fn provision_refuses_a_worker_name_that_would_leave_apps() {
    let own = tempfile::tempdir().unwrap();
    let tree = CgroupTree::locate(own.path(), "0::/\n").unwrap();
    std::fs::create_dir_all(tree.apps_dir()).unwrap();
    let worker = WorkerName::new("../..", "web", 1);

    let refused = WorkerCgroup::provision(&tree, &worker, &CgroupLimits::default());

    assert!(refused.is_err(), "a malformed name became a cgroup path");
    assert!(
        !own.path().join("..@web.1").exists(),
        "a cgroup was created outside apps/"
    );
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

    assert!(diagnostic.contains("permission denied"), "{diagnostic}");
    assert!(
        diagnostic.contains("add Delegate=yes to the riku unit (re-run riku init"),
        "remedy does not name the unit fix:\n{diagnostic}"
    );
}
