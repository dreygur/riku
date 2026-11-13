use std::cell::RefCell;

use super::*;

#[test]
fn a_failed_user_daemon_reload_fails_init_and_restarts_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let calls = RefCell::new(Vec::new());
    let systemctl = |args: &[&str]| {
        calls.borrow_mut().push(args.join(" "));
        args != ["daemon-reload"]
    };

    let started = start_user_service(&paths, UnitChange::Changed, systemctl);

    assert!(started.is_err(), "a failed reload must fail init");
    assert_eq!(
        calls.into_inner(),
        ["daemon-reload"],
        "nothing after the reload may run"
    );
}

#[test]
fn unit_delegates_its_cgroup_and_puts_no_shared_cap_on_every_app() {
    let unit = render_user_unit(&Router::Caddy, "/home/deploy/.riku");
    assert!(unit.contains("\nDelegate=yes\n"), "{unit}");
    for key in ["MemoryMax=", "CPUQuota="] {
        assert!(
            !unit.contains(key),
            "{key} on the unit caps all apps together: {unit}"
        );
    }
}
