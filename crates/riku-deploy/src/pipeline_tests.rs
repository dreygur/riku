use std::collections::HashMap;

use super::deploy_container_app;
use crate::containers::mock_state::{deploy, mock_server, DB_AND_WEB};
use crate::progress::DeployProgress;
use crate::releases::ReleaseLog;
use crate::util::deploy_logger::DeployLogger;

/// The release command fails while images are prepared, before any copy is
/// replaced: the old containers still serve, so the failure must say the
/// previous release runs and the checkout must be put back.
#[test]
fn container_prepare_failure_keeps_the_previous_release() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server(
        tmp.path(),
        "myapp",
        DB_AND_WEB,
        "RIKU_DOMAINS=myapp.example.com\n",
    );
    let app_path = server.paths.app_root.join("myapp");
    std::fs::write(app_path.join("Procfile"), "release: echo migrate\n").unwrap();
    deploy(&server).unwrap();
    ReleaseLog::new(&server.paths)
        .record("myapp", "0123456789abcdef")
        .unwrap();
    std::fs::write(server.state.join("release-exits"), "").unwrap();
    let log = DeployLogger::new(&server.paths.deploy_log_file("myapp")).unwrap();
    let mut progress = DeployProgress::new("myapp", &server.paths, log);
    let env = HashMap::from([("RIKU_DOMAINS".to_string(), "myapp.example.com".to_string())]);

    let deployed =
        deploy_container_app("myapp", &server.paths, &HashMap::new(), &env, &mut progress);

    assert!(
        deployed.is_err(),
        "the failing release must fail the deploy"
    );
    assert!(
        progress.previous_release_runs(),
        "no copy was replaced, so the previous release still runs"
    );
}
