use std::time::Duration;

use super::{restart_delay, should_restart, RestartPolicyLabel};

#[test]
fn label_values_parse() {
    assert_eq!(
        RestartPolicyLabel::try_from("no").unwrap(),
        RestartPolicyLabel::No,
        "'no' must parse to No"
    );
    assert_eq!(
        RestartPolicyLabel::try_from("always").unwrap(),
        RestartPolicyLabel::Always,
        "'always' must parse to Always"
    );
    assert_eq!(
        RestartPolicyLabel::try_from("unless-stopped").unwrap(),
        RestartPolicyLabel::UnlessStopped,
        "'unless-stopped' must parse to UnlessStopped"
    );
    assert_eq!(
        RestartPolicyLabel::try_from("on-failure").unwrap(),
        RestartPolicyLabel::OnFailure(None),
        "'on-failure' must parse to OnFailure(None)"
    );
    assert_eq!(
        RestartPolicyLabel::try_from("on-failure:3").unwrap(),
        RestartPolicyLabel::OnFailure(Some(3)),
        "'on-failure:3' must parse to OnFailure(Some(3))"
    );

    let err = RestartPolicyLabel::try_from("unknown").unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid restart policy label",
        "error must not echo untrusted label text"
    );
}

#[test]
fn on_failure_respects_limit() {
    let policy = RestartPolicyLabel::OnFailure(Some(2));
    assert!(
        should_restart(&policy, 1, 0),
        "exit 1 with 0 restarts should restart"
    );
    assert!(
        should_restart(&policy, 1, 1),
        "exit 1 with 1 restart should restart"
    );
    assert!(
        !should_restart(&policy, 1, 2),
        "exit 1 with 2 restarts must not restart"
    );
}

#[test]
fn exit_zero_not_restarted_on_failure() {
    let on_failure = RestartPolicyLabel::OnFailure(None);
    assert!(
        !should_restart(&on_failure, 0, 0),
        "clean exit code 0 must not restart under on-failure"
    );

    let always = RestartPolicyLabel::Always;
    assert!(
        should_restart(&always, 0, 0),
        "clean exit code 0 must restart under always"
    );
}

#[test]
fn backoff_caps_at_sixty_seconds() {
    assert_eq!(
        restart_delay(0),
        Duration::from_secs(1),
        "0 restarts must back off 1 s"
    );
    assert_eq!(
        restart_delay(3),
        Duration::from_secs(8),
        "3 restarts must back off 8 s"
    );
    assert_eq!(
        restart_delay(6),
        Duration::from_secs(60),
        "6 restarts must back off 60 s"
    );
    assert_eq!(
        restart_delay(20),
        Duration::from_secs(60),
        "20 restarts must cap at 60 s"
    );
}
