use std::fs;
use std::time::{Duration, Instant};

use riku_config::RikuPaths;

use crate::containers::mock_actions::mock_actions;
use crate::containers::mock_engine::{logged, mock_engine};
use crate::containers::test_ticks::wait_until;
use crate::containers::ContainerWatch;
use crate::health::actions::RestartOutcome;

const ID: &str = "1111111111111111111111111111111111111111111111111111111111111111";

#[test]
fn a_hanging_log_call_does_not_hold_up_the_watch_tick() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    fs::write(
        &state_file,
        format!(
            r#"[{{"Id":"{ID}","Name":"/myapp.web.1","Image":"sha256:abc","Config":{{"Image":"myapp:1","Labels":{{"riku.app":"myapp","riku.service":"web","riku.copy":"1"}}}},"State":{{"Status":"running","ExitCode":0,"Health":null}}}}]"#
        ),
    )
    .unwrap();
    let script = format!(
        r#"case "$1" in
           ps) echo "{ID}";;
           inspect) cat '{state}';;
           logs) sleep 3;;
           events) sleep 30;;
           esac"#,
        state = state_file.display()
    );
    let (engine, argv_log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let mut watch = ContainerWatch::with_engine(engine, paths, actions, &dir.path().join("logs"));
    let t0 = Instant::now();
    watch.tick(t0);
    wait_until(
        || {
            logged(&argv_log)
                .iter()
                .any(|line| line.starts_with("logs"))
        },
        "the log thread to start its hanging call",
    );

    let started = Instant::now();
    watch.tick(t0 + Duration::from_secs(5));

    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the tick waited {:?} on a log call",
        started.elapsed()
    );
}
