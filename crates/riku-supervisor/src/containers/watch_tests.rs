use std::fs;
use std::time::{Duration, Instant};

use riku_config::RikuPaths;

use super::mock_actions::mock_actions;
use super::mock_engine::{logged, mock_engine};
use super::test_ticks::tick_until;
use super::ContainerWatch;
use crate::health::actions::RestartOutcome;

const ID_1: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const ID_2: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn entry(id: &str, name: &str, status: &str, exit_code: Option<i32>, restart: &str) -> String {
    let exit_code_str = match exit_code {
        Some(c) => c.to_string(),
        None => "null".to_string(),
    };
    format!(
        r#"{{"Id":"{id}","Name":"/{name}","Image":"sha256:abc","Config":{{"Image":"myapp:1","Labels":{{"riku.app":"myapp","riku.service":"web","riku.copy":"1","riku.restart":"{restart}"}}}},"State":{{"Status":"{status}","ExitCode":{exit_code_str},"Health":null}}}}"#
    )
}

#[test]
fn busy_deploy_skips_restart() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let content = format!(
        "[{}]",
        entry(ID_1, "myapp-web-1", "exited", Some(1), "always")
    );
    fs::write(&state_file, content).unwrap();

    let script = format!(
        r#"case "$1" in
           ps) echo "{ID_1}";;
           inspect) cat '{state}';;
           logs) exit 0;;
           events) sleep 30;;
           esac"#,
        state = state_file.display()
    );

    let (engine, _log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Busy);
    let log_root = dir.path().join("logs");
    let mut watch = ContainerWatch::with_engine(engine, paths, actions.clone(), &log_root);

    let t0 = Instant::now();
    watch.tick(t0);

    tick_until(
        &mut watch,
        t0,
        |_| actions.call_count() >= 2,
        "a busy restart to be tried again on a later tick",
    );

    let rows = watch.rows();
    assert_eq!(rows.len(), 1, "one row for the exited copy");
    assert_eq!(
        rows[0].restart_count, 0,
        "a busy restart must not count, and is not held back by backoff"
    );
}

#[test]
fn crashed_unless_stopped_copy_is_restarted() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let content = format!(
        "[{}]",
        entry(ID_1, "myapp-web-1", "exited", Some(137), "unless-stopped")
    );
    fs::write(&state_file, content).unwrap();

    let script = format!(
        r#"case "$1" in
           ps) echo "{ID_1}";;
           inspect) cat '{state}';;
           logs) exit 0;;
           events) sleep 30;;
           esac"#,
        state = state_file.display()
    );

    let (engine, _log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let log_root = dir.path().join("logs");
    let mut watch = ContainerWatch::with_engine(engine, paths, actions.clone(), &log_root);

    tick_until(
        &mut watch,
        Instant::now(),
        |_| actions.call_count() >= 1,
        "a restart of the crashed unless-stopped copy",
    );
}

#[test]
fn die_event_triggers_restart() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let marker = dir.path().join("marker");

    let running = format!(
        "[{}]",
        entry(ID_1, "myapp-web-1", "running", None, "on-failure")
    );
    fs::write(&state_file, running).unwrap();

    let script = format!(
        r#"case "$1" in
           ps) echo "{ID_1}";;
           inspect) cat '{state}';;
           logs) exit 0;;
           events)
               while [ ! -f '{marker_path}' ]; do sleep 0.05; done
               echo '{{"Action":"die","Actor":{{"ID":"{ID_1}"}},"Type":"container"}}'
               sleep 30
               ;;
           esac"#,
        state = state_file.display(),
        marker_path = marker.display()
    );

    let (engine, log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let log_root = dir.path().join("logs");
    let mut watch = ContainerWatch::with_engine(engine, paths, actions.clone(), &log_root);

    let t0 = Instant::now();
    watch.tick(t0);

    let exited = format!(
        "[{}]",
        entry(ID_1, "myapp-web-1", "exited", Some(1), "on-failure")
    );
    fs::write(&state_file, exited).unwrap();
    fs::write(&marker, "").unwrap();

    let t5 = t0 + Duration::from_secs(5);
    tick_until(
        &mut watch,
        t5,
        |_| actions.call_count() >= 1,
        "a restart driven by the die event",
    );

    assert_eq!(actions.call_count(), 1);
    let lines = logged(&log);
    let ps_count = lines.iter().filter(|l| l.starts_with("ps")).count();
    assert_eq!(
        ps_count, 1,
        "no ps command must run after adoption; events must drive the restart: {lines:?}"
    );
}

#[test]
fn event_stream_runs_only_while_riku_containers_exist() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let ps_file = dir.path().join("ps.txt");
    fs::write(&ps_file, "").unwrap();
    fs::write(&state_file, "[]").unwrap();
    let script = format!(
        r#"case "$1" in
           ps) cat '{ps_path}';;
           inspect) cat '{state}';;
           stats) exit 0;;
           logs) exit 0;;
           events) sleep 30;;
           esac"#,
        ps_path = ps_file.display(),
        state = state_file.display()
    );
    let (engine, _log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let log_root = dir.path().join("logs");
    let mut watch = ContainerWatch::with_engine(engine, paths, actions, &log_root);
    let streaming = |watch: &ContainerWatch| matches!(watch.feed, super::Feed::Events(_));

    let t0 = Instant::now();
    watch.tick(t0);
    let idle = streaming(&watch);

    fs::write(&ps_file, format!("{ID_1}\n")).unwrap();
    let running = entry(ID_1, "myapp-web-1", "running", None, "always");
    fs::write(&state_file, format!("[{running}]")).unwrap();
    watch.tick(t0 + Duration::from_secs(31));
    let deployed = streaming(&watch);

    fs::write(&ps_file, "").unwrap();
    fs::write(&state_file, "[]").unwrap();
    watch.tick(t0 + Duration::from_secs(62));
    let destroyed = streaming(&watch);

    assert_eq!(
        (idle, deployed, destroyed),
        (false, true, false),
        "event stream open (no containers, one container, none again)"
    );
}

#[test]
fn restarted_copy_keeps_its_count() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let ps_file = dir.path().join("ps.txt");

    fs::write(&ps_file, format!("{ID_1}\n")).unwrap();
    let exited = format!(
        "[{}]",
        entry(ID_1, "myapp-web-1", "exited", Some(1), "always")
    );
    fs::write(&state_file, exited).unwrap();

    let script = format!(
        r#"case "$1" in
           ps) cat '{ps_path}';;
           inspect) cat '{state}';;
           stats) echo '{{"ID":"{ID_2}","CPUPerc":"0.00%","MemUsage":"1MiB / 1GiB"}}';;
           logs) exit 0;;
           events) sleep 30;;
           esac"#,
        ps_path = ps_file.display(),
        state = state_file.display()
    );

    let (engine, _log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let log_root = dir.path().join("logs");
    let mut watch = ContainerWatch::with_engine(engine, paths, actions.clone(), &log_root);

    let t0 = Instant::now();
    watch.tick(t0);

    tick_until(
        &mut watch,
        t0,
        |w| actions.call_count() >= 1 && !w.restarts.is_in_flight("myapp-web-1"),
        "the restart report",
    );

    fs::write(&ps_file, format!("{ID_2}\n")).unwrap();
    let running = format!(
        "[{}]",
        entry(ID_2, "myapp-web-1", "running", None, "always")
    );
    fs::write(&state_file, running).unwrap();

    let t31 = t0 + Duration::from_secs(31);
    watch.tick(t31);

    let rows = watch.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].container.as_ref().map(|c| c.id.as_str()),
        Some(ID_2),
        "container id must be updated to the new id"
    );
    assert_eq!(
        rows[0].restart_count, 1,
        "restart_count must be preserved across restarted copy"
    );
}
