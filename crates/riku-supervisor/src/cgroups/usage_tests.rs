use super::*;
use std::fs;

fn cgroup_with(files: &[(&str, &str)]) -> (tempfile::TempDir, WorkerCgroup) {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        fs::write(dir.path().join(name), text).unwrap();
    }
    let cgroup = WorkerCgroup {
        path: dir.path().to_path_buf(),
    };
    (dir, cgroup)
}

#[test]
fn usage_reads_memory_current_and_cpu_usage_usec() {
    let (_dir, cgroup) = cgroup_with(&[
        ("memory.current", "9961472\n"),
        (
            "cpu.stat",
            "usage_usec 1234567\nuser_usec 1000000\nsystem_usec 234567\n",
        ),
    ]);

    assert_eq!(
        cgroup.usage(),
        Some(ResourceUsage {
            cpu_time_ms: 1234,
            memory_bytes: 9_961_472,
        })
    );
}

/// Without the memory controller the cgroup has no `memory.current`; the
/// caller then sums the process tree instead.
#[test]
fn usage_is_none_without_memory_current() {
    let (_dir, cgroup) = cgroup_with(&[("cpu.stat", "usage_usec 10\n")]);

    assert_eq!(cgroup.usage(), None);
}
