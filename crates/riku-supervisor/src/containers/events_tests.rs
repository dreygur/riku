use std::fs;
use std::time::{Duration, Instant};

use riku_config::RikuPaths;

use super::mock_actions::mock_actions;
use super::mock_engine::mock_engine;
use super::ContainerWatch;
use crate::health::actions::RestartOutcome;

const OLD: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const NEW: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn running(id: &str) -> String {
    format!(
        r#"[{{"Id":"{id}","Name":"/myapp.web.1","Image":"sha256:abc","Config":{{"Image":"myapp:1","Labels":{{"riku.app":"myapp","riku.service":"web","riku.copy":"1"}}}},"State":{{"Status":"running","ExitCode":0,"Health":null}}}}]"#
    )
}

fn row_ids(watch: &ContainerWatch) -> Vec<String> {
    watch
        .rows()
        .into_iter()
        .filter_map(|row| row.container.map(|c| c.id))
        .collect()
}

#[test]
fn replaced_container_leaves_the_rows_before_the_next_tick() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let replaced = dir.path().join("replaced");
    fs::write(&state_file, running(OLD)).unwrap();
    let script = format!(
        r#"case "$1" in
           ps) echo "{OLD}";;
           inspect) cat '{state}';;
           logs) exit 0;;
           events)
               while [ ! -f '{replaced}' ]; do sleep 0.05; done
               echo '{{"Action":"destroy","Actor":{{"ID":"{OLD}"}},"Type":"container"}}'
               echo '{{"Action":"start","Actor":{{"ID":"{NEW}"}},"Type":"container"}}'
               sleep 30
               ;;
           esac"#,
        state = state_file.display(),
        replaced = replaced.display()
    );
    let (engine, _log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let mut watch = ContainerWatch::with_engine(engine, paths, actions, &dir.path().join("logs"));
    watch.tick(Instant::now());
    assert_eq!(row_ids(&watch), [OLD]);

    fs::write(&state_file, running(NEW)).unwrap();
    fs::write(&replaced, "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !watch.apply_events() {
        assert!(Instant::now() < deadline, "no event arrived within 5 s");
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(row_ids(&watch), [NEW], "the old container id must be gone");
}

#[test]
fn requested_listing_finds_the_first_copy_before_the_interval() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let ps_file = dir.path().join("ps.txt");
    fs::write(&ps_file, "").unwrap();
    fs::write(&state_file, "[]").unwrap();
    let script = format!(
        r#"case "$1" in
           ps) cat '{ps}';;
           inspect) cat '{state}';;
           stats) exit 0;;
           logs) exit 0;;
           events) sleep 30;;
           esac"#,
        ps = ps_file.display(),
        state = state_file.display()
    );
    let (engine, _log) = mock_engine(dir.path(), &script);
    let paths = RikuPaths::for_tests(dir.path());
    let actions = mock_actions(RestartOutcome::Restarted);
    let mut watch = ContainerWatch::with_engine(engine, paths, actions, &dir.path().join("logs"));
    let t0 = Instant::now();
    watch.tick(t0);

    fs::write(&ps_file, format!("{OLD}\n")).unwrap();
    fs::write(&state_file, running(OLD)).unwrap();
    let t5 = t0 + Duration::from_secs(5);
    watch.tick(t5);
    let unrequested = row_ids(&watch);
    watch.request_listing();
    watch.tick(t5);

    assert_eq!(
        (unrequested, row_ids(&watch)),
        (vec![], vec![OLD.to_string()]),
        "the copy must show only once a listing is requested, before the 30 s listing"
    );
    assert!(
        matches!(watch.feed, super::Feed::Events(_)),
        "the requested listing must open the event stream"
    );
}
