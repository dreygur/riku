use super::*;

const PROC_SELF_CGROUP: &str = "12:pids:/system.slice/riku.service\n\
                                0::/system.slice/riku.service\n";

#[test]
fn locates_the_v2_line_of_a_service_under_the_mount() {
    let tree = CgroupTree::locate(Path::new("/sys/fs/cgroup"), PROC_SELF_CGROUP).unwrap();
    assert_eq!(
        tree.own,
        Path::new("/sys/fs/cgroup/system.slice/riku.service")
    );
}

#[test]
fn locates_the_root_cgroup_as_the_mount_itself() {
    let tree = CgroupTree::locate(Path::new("/sys/fs/cgroup"), "0::/\n").unwrap();
    assert_eq!(tree.own, Path::new("/sys/fs/cgroup"));
}

#[test]
fn refuses_a_host_without_cgroup_v2_or_a_path_that_climbs() {
    let v1_only = CgroupTree::locate(Path::new("/sys/fs/cgroup"), "4:memory:/user.slice\n");
    assert!(v1_only.is_err(), "no 0:: line must be an error");

    let climbing = CgroupTree::locate(Path::new("/sys/fs/cgroup"), "0::/a/../../etc\n");
    assert!(climbing.is_err(), "a .. component must be refused");
}

/// A plain directory stands in for the delegated cgroup, so this checks the
/// layout and the writes, not what the kernel does with them. That is only
/// checked on a real systemd host.
#[test]
fn take_over_moves_the_pid_and_enables_only_offered_controllers() {
    let own = tempfile::tempdir().unwrap();
    fs::write(own.path().join("cgroup.controllers"), "cpuset cpu pids\n").unwrap();
    let mut tree = CgroupTree::locate(own.path(), "0::/\n").unwrap();

    tree.take_over(4242).unwrap();

    assert_eq!(tree.missing_controllers(), ["memory"]);
    let read = |rel: &str| fs::read_to_string(own.path().join(rel)).unwrap();
    assert_eq!(read("supervisor/cgroup.procs"), "4242");
    assert_eq!(read("cgroup.subtree_control"), "+cpu +pids");
    assert_eq!(read("apps/cgroup.subtree_control"), "+cpu +pids");
}
