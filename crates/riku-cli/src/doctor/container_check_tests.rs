use super::buildx_check;
use crate::doctor::Status;

#[test]
fn docker_without_buildx_warns_with_the_install_hint() {
    let check = buildx_check(Some(false)).expect("a check for docker without buildx");

    assert_eq!(check.status, Status::Warn);
    assert!(
        check.detail.contains("docker-buildx-plugin"),
        "no install hint: {}",
        check.detail
    );
}

#[test]
fn buildx_is_ok_when_present_and_not_checked_for_other_engines() {
    assert_eq!(buildx_check(Some(true)).map(|c| c.status), Some(Status::Ok));
    assert!(buildx_check(None).is_none(), "podman got a buildx check");
}
