use super::*;
use tempfile::{NamedTempFile, TempDir};

#[test]
fn a_recorded_deploy_error_is_read_back_for_the_push() {
    let status = NamedTempFile::new().unwrap();
    assert_eq!(
        deploy_result(status.path()),
        DeployResult::Succeeded,
        "an untouched status file means the deploy did not fail"
    );

    let err = anyhow::anyhow!("deploy already in progress for app 'c2'");
    write_failure(status.path(), &err).unwrap();

    assert_eq!(
        deploy_result(status.path()),
        DeployResult::Failed("deploy already in progress for app 'c2'".to_string())
    );
}

#[test]
fn the_hook_never_creates_a_status_file() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("status");

    let written = write_failure(&missing, &anyhow::anyhow!("boom"));

    assert!(written.is_err(), "writing must fail for a missing file");
    assert!(!missing.exists(), "the hook created {}", missing.display());
}
