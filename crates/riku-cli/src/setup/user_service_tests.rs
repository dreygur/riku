use super::*;

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
