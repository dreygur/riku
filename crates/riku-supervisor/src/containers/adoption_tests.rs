use std::fs;
use std::time::{Duration, Instant};

use riku_config::RikuPaths;

use super::mock_actions::mock_actions;
use super::mock_engine::{logged, mock_engine, mock_engine_without_events};
use super::test_ticks::{tick_until, wait_until};
use super::ContainerWatch;
use crate::health::actions::RestartOutcome;

const ID_1: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const ID_2: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn entry(id: &str, name: &str, copy: &str, status: &str, exit_code: Option<i32>) -> String {
    let exit_code_str = match exit_code {
        Some(c) => c.to_string(),
        None => "null".to_string(),
    };
    format!(
        r#"{{"Id":"{id}","Name":"/{name}","Image":"sha256:abc","Config":{{"Image":"myapp:1","Labels":{{"riku.app":"myapp","riku.service":"web","riku.copy":"{copy}","riku.restart":"always"}}}},"State":{{"Status":"{status}","ExitCode":{exit_code_str},"Health":null}}}}"#
    )
}

#[test]
fn adoption_never_stops_copies() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let content = format!(
        "[{},{}]",
        entry(ID_1, "myapp-web-1", "1", "running", None),
        entry(ID_2, "myapp-web-2", "2", "running", None)
    );
    fs::write(&state_file, content).unwrap();

    let script = format!(
        r#"case "$1" in
           ps) printf '%s\n%s\n' "{ID_1}" "{ID_2}";;
           inspect) cat '{state}';;
           logs) exit 0;;
           events) sleep 30;;
           esac"#,
        state = state_file.display()
    );

    let (engine, log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let log_root = dir.path().join("logs");
    let mut watch = ContainerWatch::with_engine(engine, paths, actions, &log_root);

    let now = Instant::now();
    watch.tick(now);

    let starts_after_newest = |id: &str| {
        let newest_line = format!("logs --timestamps --tail 1 {id}");
        logged(&log).contains(&newest_line)
    };
    wait_until(
        || starts_after_newest(ID_1) && starts_after_newest(ID_2),
        "both adopted copies to start after their newest log line",
    );
    let lines = logged(&log);
    for line in &lines {
        assert!(!line.starts_with("stop"), "unexpected stop in log: {line}");
        assert!(!line.starts_with("kill"), "unexpected kill in log: {line}");
        assert!(!line.starts_with("rm"), "unexpected rm in log: {line}");
    }
}

#[test]
fn no_events_falls_back_to_polling() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let running = format!("[{}]", entry(ID_1, "myapp-web-1", "1", "running", None));
    fs::write(&state_file, running).unwrap();

    let script = format!(
        r#"case "$1" in
           ps) echo "{ID_1}";;
           inspect) cat '{state}';;
           logs) exit 0;;
           esac"#,
        state = state_file.display()
    );

    let (engine, _log) = mock_engine_without_events(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let log_root = dir.path().join("logs");
    let mut watch = ContainerWatch::with_engine(engine, paths, actions.clone(), &log_root);

    let t0 = Instant::now();
    watch.tick(t0);

    let exited = format!("[{}]", entry(ID_1, "myapp-web-1", "1", "exited", Some(1)));
    fs::write(&state_file, exited).unwrap();

    let t5 = t0 + Duration::from_secs(5);
    watch.tick(t5);
    assert_eq!(
        actions.call_count(),
        0,
        "tick at +5 s must not restart before polling interval"
    );

    tick_until(
        &mut watch,
        t0 + Duration::from_secs(31),
        |_| actions.call_count() >= 1,
        "a restart found by the 30 s listing",
    );
    assert_eq!(actions.call_count(), 1);
}
