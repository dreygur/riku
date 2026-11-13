use crate::containers::mock_state::{
    deploy, deploy_keeping_output, mock_server_with_domain, MockServer, DB_AND_WEB,
};
use crate::util::display::OutputTail;

const DOMAIN: &str = "myapp.example.com";

fn position(calls: &[String], wanted: impl Fn(&str) -> bool) -> usize {
    calls
        .iter()
        .position(|line| wanted(line))
        .unwrap_or_else(|| panic!("no matching call in {calls:#?}"))
}

#[test]
fn deploy_starts_services_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    server.mark("slow", "db");

    deploy(&server).unwrap();

    let calls = server.calls();
    let db_ready = position(&calls, |l| l == "ready myapp.db.1");
    let web_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp.web.1")
    });
    assert!(
        db_ready < web_run,
        "web started before db was healthy: {calls:#?}"
    );
    assert_eq!(server.containers(), ["myapp.db.1", "myapp.web.1"]);
}

#[test]
fn redeploy_keeps_old_copy_on_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    deploy(&server).unwrap();
    let site_before = server.site("myapp");
    assert!(site_before.contains("reverse_proxy"), "{site_before}");
    server.mark("exits", "web");
    std::fs::write(server.state.join("image-id"), "sha256:new\n").unwrap();

    let err = deploy(&server).unwrap_err();

    assert!(format!("{err:#}").contains("exited with code 1"), "{err:#}");
    assert!(
        server.containers().contains(&"myapp.web.1".to_string()),
        "old web copy is gone: {:?}",
        server.containers()
    );
    assert!(
        !server.containers().contains(&"myapp.web.2".to_string()),
        "failed copy left behind: {:?}",
        server.containers()
    );
    assert_eq!(server.site("myapp"), site_before, "router config changed");
}

fn with_release(server: &MockServer) {
    let app_path = server.paths.app_root.join("myapp");
    std::fs::write(app_path.join("Procfile"), "release: echo migrate\n").unwrap();
}

#[test]
fn release_runs_in_new_image_before_rollout() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    with_release(&server);

    deploy(&server).unwrap();

    let calls = server.calls();
    let release_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp-release")
    });
    let release_wait = position(&calls, |l| l.starts_with("wait"));
    let release_rm = position(&calls, |l| l.starts_with("rm"));
    let web_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp.web.1")
    });
    let run_line = &calls[release_run];
    assert!(
        run_line.contains("--entrypoint /bin/sh")
            && run_line.contains("nginx:alpine -c echo migrate"),
        "release did not run through the shell in the web image: {run_line}"
    );
    assert!(
        release_run < release_wait && release_wait < release_rm && release_rm < web_run,
        "release run, wait and rm did not all come before the web copy: {calls:#?}"
    );
    assert_eq!(server.containers(), ["myapp.db.1", "myapp.web.1"]);
}

#[test]
fn failing_release_aborts_before_rollout() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    with_release(&server);
    deploy(&server).unwrap();
    std::fs::write(server.state.join("release-exits"), "").unwrap();

    let err = deploy(&server).unwrap_err();

    let message = format!("{err:#}");
    assert!(message.contains("exited with code 1"), "{message}");
    let web_runs = server
        .calls()
        .iter()
        .filter(|l| l.starts_with("run") && l.contains("myapp.web."))
        .count();
    assert_eq!(
        web_runs, 1,
        "a new web copy started after the failed release"
    );
    assert_eq!(server.containers(), ["myapp.db.1", "myapp.web.1"]);
}

#[test]
fn first_deploy_starts_the_release_dependencies_before_the_release() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    with_release(&server);

    deploy(&server).unwrap();

    let calls = server.calls();
    let db_ready = position(&calls, |l| l == "ready myapp.db.1");
    let release_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp-release")
    });
    assert!(
        db_ready < release_run,
        "the release ran before db was up: {calls:#?}"
    );
    let db_runs = calls
        .iter()
        .filter(|l| l.starts_with("run") && l.contains("myapp.db."))
        .count();
    assert_eq!(db_runs, 1, "db started twice on one deploy: {calls:#?}");
    assert_eq!(server.containers(), ["myapp.db.1", "myapp.web.1"]);
}

#[test]
fn redeploy_leaves_a_running_release_dependency_alone_for_the_release() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    with_release(&server);
    deploy(&server).unwrap();
    let before = server.calls().len();

    deploy(&server).unwrap();

    let calls = server.calls()[before..].to_vec();
    let release_run = position(&calls, |l| {
        l.starts_with("run") && l.contains("myapp-release")
    });
    let touched_db = calls[..release_run]
        .iter()
        .any(|l| l.starts_with("stop") || (l.starts_with("run") && l.contains("myapp.db.")));
    assert!(
        !touched_db,
        "db was stopped or started again before the release: {calls:#?}"
    );
}

#[test]
fn failing_release_output_reaches_the_rail_tail_from_both_streams() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    with_release(&server);
    std::fs::write(server.state.join("release-exits"), "").unwrap();
    let recent = OutputTail::default();

    let err = deploy_keeping_output(&server, &recent).unwrap_err();

    let lines = recent.lines();
    assert!(format!("{err:#}").contains("exited with code 1"), "{err:#}");
    assert!(
        lines.iter().any(|l| l == "Traceback: cannot reach redis"),
        "the release's stderr is missing from the streamed output: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l == "release ran"),
        "the release's stdout is missing from the streamed output: {lines:?}"
    );
}

#[test]
fn broken_settings_fail_the_deploy() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    std::fs::write(
        &server.paths.settings_file,
        "[engine]\ncall_timeout = \"soon\"\n",
    )
    .unwrap();

    let err = deploy(&server).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("settings.toml"),
        "error does not name settings.toml: {message}"
    );
    assert!(
        message.contains("call_timeout"),
        "error does not name call_timeout: {message}"
    );
    assert!(
        !server.calls().iter().any(|l| l.starts_with("run")),
        "an engine call line started with run: {:?}",
        server.calls()
    );
}

/// `riku apps` counts a container app's running rows in stats.json, which
/// the supervisor rewrites seconds later; the deploy puts them there itself
/// and leaves the other apps' entries as they were.
#[test]
fn a_container_deploy_puts_its_running_copies_in_the_stats_file() {
    use crate::supervisor::stats::{read_stats_file, ProcessStatus, StatsManager};

    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, DOMAIN);
    let stats_file = server.paths.riku_root.join("stats.json");
    let mut other = StatsManager::new();
    other.register_process("shop@web.1".into(), "shop".into(), "web".into(), 1);
    other.mark_running("shop@web.1", 4242);
    other.write_stats_to_file(&stats_file).unwrap();
    let shop_before = read_stats_file(&stats_file).unwrap();

    deploy(&server).unwrap();

    let stats = read_stats_file(&stats_file).expect("stats.json parses");
    let running: Vec<&str> = stats
        .iter()
        .filter(|entry| entry.app == "myapp")
        .flat_map(|entry| &entry.processes)
        .filter(|p| p.container.is_some() && p.status == ProcessStatus::Running)
        .map(|p| p.process_id.as_str())
        .collect();
    assert_eq!(running.len(), 2, "running container rows: {running:?}");
    let shop_after: Vec<_> = stats.iter().filter(|entry| entry.app == "shop").collect();
    assert_eq!(
        serde_json::to_string(&shop_after).unwrap(),
        serde_json::to_string(&shop_before.iter().collect::<Vec<_>>()).unwrap(),
        "another app's entry changed"
    );
}
