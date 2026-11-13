use super::*;

fn refused(group: Option<&str>) -> SocketAccess {
    SocketAccess::Refused {
        group: group.map(str::to_string),
    }
}

#[test]
fn asks_only_for_docker_on_a_terminal_when_a_group_would_grant_the_socket() {
    let docker_group = refused(Some("docker"));

    assert_eq!(
        step(EngineKind::Docker, &docker_group, true),
        Step::Ask("docker".to_string())
    );
    assert_eq!(
        step(EngineKind::Docker, &docker_group, false),
        Step::Explain,
        "without a terminal nothing changes and the choices are printed"
    );
    assert_eq!(
        step(EngineKind::Docker, &refused(None), true),
        Step::Explain,
        "a root owned socket or an unknown group gets no question"
    );
}

#[test]
fn says_nothing_for_another_engine_or_a_socket_already_reachable_or_missing() {
    for (kind, access) in [
        (EngineKind::Other, refused(Some("docker"))),
        (EngineKind::Docker, SocketAccess::Reachable),
        (EngineKind::Other, SocketAccess::Missing),
    ] {
        assert_eq!(
            step(kind, &access, true),
            Step::Nothing,
            "{kind:?} {access:?}"
        );
    }
}

/// Docker is installed but its daemon is not running, so there is no socket
/// to judge: init says so instead of staying silent.
#[test]
fn docker_without_a_socket_says_it_is_not_running() {
    for tty in [true, false] {
        assert_eq!(
            step(EngineKind::Docker, &SocketAccess::Missing, tty),
            Step::NotRunning,
            "tty {tty}"
        );
    }
}

#[test]
fn only_an_explicit_yes_adds_the_group() {
    for yes in ["y", "Y", "yes", " YES\n"] {
        assert!(accepted(yes), "{yes:?} must count as yes");
    }
    for no in ["", "\n", "n", "no", "maybe", "yess"] {
        assert!(!accepted(no), "{no:?} must count as no");
    }
}

#[test]
fn only_a_program_named_docker_is_docker() {
    assert_eq!(engine_kind("docker"), EngineKind::Docker);
    for other in ["podman", "nerdctl", "finch", "docker-rootless"] {
        assert_eq!(engine_kind(other), EngineKind::Other, "{other}");
    }
}

/// A unix socket opens for whoever may write it: the owner through the
/// user bits, a member of its group through the group bits, anyone else
/// through the other bits.
#[test]
fn socket_write_access_follows_the_permission_bits() {
    let (root, deploy) = (0, 1001);

    assert!(may_write(root, 0o660, deploy, true));
    assert!(!may_write(root, 0o660, deploy, false));
    assert!(!may_write(root, 0o600, deploy, true));
    assert!(may_write(deploy, 0o600, deploy, false));
    assert!(may_write(root, 0o666, deploy, false));
}

#[test]
fn the_group_is_named_only_when_joining_it_would_grant_the_socket() {
    assert_eq!(
        joinable_group(998, Some("docker"), 0o660),
        Some("docker".to_string())
    );
    assert_eq!(joinable_group(0, Some("root"), 0o660), None, "root group");
    assert_eq!(joinable_group(998, None, 0o660), None, "unresolved gid");
    assert_eq!(
        joinable_group(998, Some("docker"), 0o600),
        None,
        "no group write bit"
    );
    assert_eq!(
        joinable_group(998, Some("-o"), 0o660),
        None,
        "not a plain name"
    );
}

#[test]
fn the_question_names_the_group_and_what_it_grants() {
    let question = question("docker", "riku");

    assert!(
        question.contains("Add riku to the docker group"),
        "{question}"
    );
    assert!(question.contains("root-equivalent"), "{question}");
    assert!(question.ends_with("[y/N] "), "{question}");
}
