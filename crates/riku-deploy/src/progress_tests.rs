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

#[test]
fn durations_switch_to_minutes_at_sixty_seconds() {
    assert_eq!(format_duration(Duration::from_millis(12_400)), "12.4s");
    assert_eq!(format_duration(Duration::from_secs(123)), "2m 03s");
}
