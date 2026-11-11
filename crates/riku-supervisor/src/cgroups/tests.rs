use super::*;

/// A tempdir standing in for the supervisor's own cgroup, taken over with
/// `offered` as its `cgroup.controllers`.
fn taken_over(offered: &str) -> (tempfile::TempDir, CgroupTree) {
    let own = tempfile::tempdir().unwrap();
    std::fs::write(own.path().join("cgroup.controllers"), offered).unwrap();
    let mut tree = CgroupTree::locate(own.path(), "0::/\n").unwrap();
    tree.take_over(1).unwrap();
    (own, tree)
}

#[test]
fn provision_writes_only_the_limits_that_are_set() {
    let (_own, tree) = taken_over("memory cpu pids");
    let limits = CgroupLimits {
        memory_max_bytes: Some(1024),
        cpu_period_us: 100_000,
        ..CgroupLimits::default()
    };

    let cgroup =
        WorkerCgroup::provision(&tree, &WorkerName::new("shop", "web", 1), &limits).unwrap();

    assert!(cgroup.path.join("memory.max").exists());
    for node in ["cpu.max", "pids.max"] {
        assert!(
            !cgroup.path.join(node).exists(),
            "{node} written without a limit"
        );
    }
}

/// A worker cgroup dir left from an older config keeps its files. A limit
/// the new config no longer sets must go back to unlimited, not stay.
#[test]
fn reprovision_resets_limits_the_new_config_dropped() {
    let (_own, tree) = taken_over("memory cpu pids");
    let worker = WorkerName::new("shop", "web", 1);
    let old = CgroupLimits {
        memory_max_bytes: Some(1024),
        cpu_quota_us: Some(20_000),
        cpu_period_us: 50_000,
        pids_max: Some(16),
    };
    WorkerCgroup::provision(&tree, &worker, &old).unwrap();
    let new = CgroupLimits {
        memory_max_bytes: Some(2048),
        cpu_period_us: 50_000,
        ..CgroupLimits::default()
    };

    let cgroup = WorkerCgroup::provision(&tree, &worker, &new).unwrap();

    for (node, want) in [
        ("memory.max", "2048"),
        ("cpu.max", "max 50000"),
        ("pids.max", "max"),
    ] {
        let got = std::fs::read_to_string(cgroup.path.join(node)).unwrap();
        assert_eq!(got, want, "{node}");
    }
}

#[test]
fn provision_refuses_a_limit_whose_controller_is_not_delegated() {
    let (own, tree) = taken_over("cpu pids");
    let limits = CgroupLimits {
        memory_max_bytes: Some(1024),
        ..CgroupLimits::default()
    };

    let refused = WorkerCgroup::provision(&tree, &WorkerName::new("shop", "web", 1), &limits);

    let message = format!(
        "{:#}",
        refused.err().expect("a memory limit without memory ran")
    );
    assert!(message.contains("memory"), "{message}");
    assert!(!own.path().join("apps/shop@web.1").exists());
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
