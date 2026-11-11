use super::*;

#[test]
fn units_follow_router() {
    let caddy = render_system_unit(&Router::Caddy, "deploy", "/home/deploy", "/usr/bin/riku");
    assert!(
        caddy.contains("After=network.target caddy.service\nWants=caddy.service\n"),
        "{caddy}"
    );
    assert!(
        !caddy.contains("nginx"),
        "caddy host unit mentions nginx: {caddy}"
    );

    let nginx = render_system_unit(&Router::Nginx, "deploy", "/home/deploy", "/usr/bin/riku");
    assert!(
        nginx.contains("After=network.target nginx.service\nWants=nginx.service\n"),
        "{nginx}"
    );
}

#[test]
fn unit_delegates_its_cgroup_to_the_supervisor() {
    let unit = render_system_unit(&Router::Nginx, "deploy", "/home/deploy", "/usr/bin/riku");
    assert!(
        unit.contains("\nDelegate=yes\n"),
        "without Delegate=yes isolated workers cannot get a cgroup: {unit}"
    );
}

#[test]
fn unit_puts_no_shared_cap_on_every_app() {
    let unit = render_system_unit(&Router::Nginx, "deploy", "/home/deploy", "/usr/bin/riku");
    for key in ["MemoryMax=", "CPUQuota="] {
        assert!(
            !unit.contains(key),
            "{key} on the unit caps all apps together: {unit}"
        );
    }
}

#[test]
fn directories_outside_the_home_stay_writable() {
    let unit = render_system_unit(&Router::Nginx, "deploy", "/home/deploy", "/usr/bin/riku");
    assert!(
        unit.contains(
            "ReadWritePaths=/home/deploy/.riku -/run/riku-sockets \
             -/var/lib/riku/acme -/var/lib/riku/static\n"
        ),
        "ProtectSystem=strict would leave a directory riku writes outside the home read only: {unit}"
    );
}
