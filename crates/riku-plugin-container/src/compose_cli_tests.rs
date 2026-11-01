use super::*;

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
