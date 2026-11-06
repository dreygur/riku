use std::path::Path;
use std::time::Duration;

use super::run_args;
use super::tests::argv;
use crate::engine::fixture::sample_spec;
use crate::types::{HealthCheck, HealthTest};

fn health(test: HealthTest) -> HealthCheck {
    HealthCheck {
        test,
        interval: None,
        timeout: None,
        retries: None,
        start_period: None,
    }
}

#[test]
fn entrypoint_and_health_command_starting_with_a_dash_stay_flag_values() {
    let mut spec = sample_spec();
    spec.entrypoint = Some(vec!["-sh".into()]);
    spec.healthcheck = Some(health(HealthTest::Shell("-x".into())));

    let args = argv(&spec);

    for flag in ["--entrypoint", "--health-cmd"] {
        let at = args.iter().position(|a| a == flag).unwrap();
        assert!(args[at + 1].starts_with('-'), "{args:?}");
        assert!(
            at + 1
                < args
                    .iter()
                    .position(|a| a == "registry.example/myapp:1")
                    .unwrap()
        );
    }
}

#[test]
fn exec_health_check_without_a_program_or_with_nul_is_an_error() {
    let mut spec = sample_spec();
    for exec in [vec![], vec!["curl\0".to_string()]] {
        spec.healthcheck = Some(health(HealthTest::Exec(exec.clone())));
        assert!(
            run_args(&spec, Path::new("/scratch/env-1")).is_err(),
            "{exec:?} was accepted"
        );
    }
}

#[test]
fn health_check_renders_shell_exec_and_disabled_forms() {
    let mut spec = sample_spec();
    spec.healthcheck = Some(HealthCheck {
        interval: Some(Duration::from_secs(10)),
        retries: Some(3),
        start_period: Some(Duration::from_secs(5)),
        ..health(HealthTest::Exec(vec![
            "curl".into(),
            "-f".into(),
            "http://x/a b".into(),
        ]))
    });
    let args = argv(&spec).join(" ");
    assert!(
        args.contains("--health-cmd curl -f 'http://x/a b'"),
        "{args}"
    );
    assert!(args.contains("--health-interval 10s"), "{args}");
    assert!(args.contains("--health-retries 3"), "{args}");
    assert!(args.contains("--health-start-period 5s"), "{args}");

    spec.healthcheck = Some(health(HealthTest::Disabled));
    assert!(argv(&spec).contains(&"--no-healthcheck".to_string()));
}

#[test]
fn sub_second_durations_round_up_to_a_whole_second() {
    // `0s` would mean the engine default (30 s) for a health interval and an
    // immediate kill for the stop grace.
    let mut spec = sample_spec();
    spec.stop_grace = Duration::from_millis(500);
    spec.healthcheck = Some(HealthCheck {
        interval: Some(Duration::from_millis(500)),
        timeout: Some(Duration::from_millis(1500)),
        ..health(HealthTest::Shell("true".into()))
    });

    let args = argv(&spec).join(" ");

    assert!(args.contains("--stop-timeout 1 "), "{args}");
    assert!(args.contains("--health-interval 1s"), "{args}");
    assert!(args.contains("--health-timeout 2s"), "{args}");
}
