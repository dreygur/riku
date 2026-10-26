use super::*;

#[test]
fn compose_cmd_display_formats_with_base_args() {
    assert_eq!(
        compose_cmd_display(&("docker", vec!["compose"])),
        "docker compose"
    );
    assert_eq!(
        compose_cmd_display(&("podman-compose", vec![])),
        "podman-compose"
    );
}

#[test]
fn pull_leaves_buildable_services_to_compose_build() {
    assert_eq!(pull_args("docker"), ["pull", "--ignore-buildable"]);
    assert_eq!(pull_args("podman"), ["pull", "--ignore-buildable"]);
    assert_eq!(
        pull_args("podman-compose"),
        ["pull"],
        "podman-compose skips buildable services itself and rejects the flag"
    );
}
