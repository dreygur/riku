use std::collections::HashMap;

use super::deploy_container_app;
use crate::containers::mock_state::{deploy, mock_server_with_domain, DB_AND_WEB};
use crate::mock_engine::socket_refusing_engine;
use crate::progress::DeployProgress;
use crate::releases::ReleaseLog;
use crate::util::deploy_logger::DeployLogger;

/// An engine the deploy user cannot reach stops a container deploy before
/// it changes anything: no pre-deploy hook runs, and the error names the
/// operator's two choices.
#[test]
fn container_deploy_stops_first_when_the_engine_socket_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
    let checkout = paths.app_root.join("shop");
    std::fs::create_dir_all(&checkout).unwrap();
    std::fs::write(
        checkout.join("compose.yaml"),
        "services:\n  web:\n    image: nginx\n",
    )
    .unwrap();
    for args in [
        &["init", "-q"][..],
        &["add", "."][..],
        &[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "c",
        ][..],
    ] {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .args(args)
            .env_remove("GIT_DIR")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }
    let engine = socket_refusing_engine(tmp.path());
    std::fs::write(&paths.container_engine_file, engine.display().to_string()).unwrap();

    let err = crate::do_deploy("shop", &paths, &HashMap::new(), None).unwrap_err();

    assert!(format!("{err:#}").contains("rootless podman"), "{err:#}");
    let log = std::fs::read_to_string(paths.deploy_log_file("shop")).unwrap_or_default();
    assert!(
        !log.contains("pre-deploy"),
        "hooks ran before the engine check:\n{log}"
    );
}

/// The release command fails while images are prepared, before any copy is
/// replaced: the old containers still serve, so the failure must say the
/// previous release runs and the checkout must be put back.
#[test]
fn container_prepare_failure_keeps_the_previous_release() {
    let tmp = tempfile::tempdir().unwrap();
    let server = mock_server_with_domain(tmp.path(), "myapp", DB_AND_WEB, "myapp.example.com");
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
