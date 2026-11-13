use super::*;
use crate::util::display::render_summary;

fn commit() -> Commit {
    Commit {
        short_sha: "54ba34f".to_string(),
        subject: "Fix login".to_string(),
    }
}

#[test]
fn success_summary_lists_what_the_pusher_needs() {
    let commit = commit();
    let urls = ["https://agenttest.example".to_string()];
    let outcome = Outcome {
        commit: Some(&commit),
        urls: &urls,
        workers: "web 1, worker 2",
    };
    let rows = success_summary("agenttest", &outcome);
    assert_eq!(
        render_summary(&as_pairs(&rows), false),
        "\n   App       agenttest\
         \n   Commit    54ba34f  Fix login\
         \n   URL       https://agenttest.example\
         \n   Workers   web 1, worker 2\
         \n   Logs      rk logs agenttest"
    );
    assert_eq!(
        intro_title("agenttest", Some(&commit)),
        "Deploying agenttest  54ba34f \"Fix login\""
    );
}

#[test]
fn failure_says_how_to_retry_and_whether_the_old_release_runs() {
    let rows = failure_summary("agenttest", "main");
    assert_eq!(
        render_summary(&as_pairs(&rows), false),
        "\n   Full log   rk logs agenttest --deploy\
         \n   Retry      git push riku main"
    );
    assert_eq!(failure_outro(false), "Deploy failed.");
    assert_eq!(
        without_label(
            "Build failed: plugin 'demo' exited with code 1",
            "Build failed"
        ),
        "plugin 'demo' exited with code 1",
        "the reason must not repeat the step label"
    );
}

#[test]
fn failure_block_puts_the_failed_step_before_the_outro_once() {
    let block = render_failure(
        "Build failed",
        "plugin 'node' exited with code 127\nsee the build output above",
        false,
        false,
    );

    assert_eq!(
        block,
        "■  Build failed  plugin 'node' exited with code 127\
         \n│  see the build output above\
         \n│\
         \n└  Deploy failed."
    );
}

/// `riku logs <app> --deploy` shows the whole build, once, whether the
/// deploy worked or not, and a failure's error comes after the output.
#[test]
fn the_deploy_log_holds_every_output_line_once_on_success_and_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let log_path = paths.deploy_log_file("agenttest");
    let lines: Vec<String> = (0..30).map(|n| format!("compiling unit {n}")).collect();
    let outcome = Outcome {
        commit: None,
        urls: &[],
        workers: "",
    };

    let mut progress =
        DeployProgress::new("agenttest", &paths, DeployLogger::new(&log_path).unwrap());
    lines.iter().for_each(|line| progress.output().push(line));
    progress.succeeded(&outcome);
    let written = std::fs::read_to_string(&log_path).unwrap();
    for line in &lines {
        assert_eq!(
            written.matches(&format!("| {line}\n")).count(),
            1,
            "{line}: {written}"
        );
    }

    let mut progress =
        DeployProgress::new("agenttest", &paths, DeployLogger::new(&log_path).unwrap());
    progress
        .output()
        .push("redis.exceptions.ConnectionError: Error 111");
    progress.failed(
        &anyhow::anyhow!("release command exited with code 1"),
        "main",
    );
    let written = std::fs::read_to_string(&log_path).unwrap();
    assert_eq!(
        written.matches("ConnectionError").count(),
        1,
        "the output is not written twice: {written}"
    );
    let reason = written.find("redis.exceptions.ConnectionError").unwrap();
    let error = written.find("[ERROR] release command exited").unwrap();
    assert!(
        reason < error,
        "the output should come before the error line: {written}"
    );
    assert!(
        written.contains("compiling unit 29"),
        "the previous deploy is kept: {written}"
    );
}

#[test]
fn durations_switch_to_minutes_at_sixty_seconds() {
    assert_eq!(format_duration(Duration::from_millis(12_400)), "12.4s");
    assert_eq!(format_duration(Duration::from_secs(123)), "2m 03s");
}
