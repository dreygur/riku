use super::*;

/// The supervisor runs these on its control plane; the CLI functions behind
/// them exit the process for an unknown app, which would stop the supervisor.
#[test]
fn an_unknown_app_is_an_error_not_a_supervisor_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let actions = CliControlActions;

    for (action, result) in [
        ("deploy", actions.deploy(&paths, "ghost")),
        ("restart", actions.restart(&paths, "ghost")),
        ("stop", actions.stop(&paths, "ghost")),
        ("destroy", actions.destroy(&paths, "ghost")),
    ] {
        let error = format!("{:#}", result.expect_err(action));
        assert!(error.contains("app 'ghost' not found"), "{action}: {error}");
    }
}
