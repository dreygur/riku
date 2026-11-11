use std::collections::HashMap;
use std::fs;

use crate::config::RikuPaths;
use crate::workers::create_workers_generic;

#[test]
fn bad_scaling_value_fails_before_any_worker_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let app_path = paths.app_root.join("myapp");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    fs::write(app_path.join("Procfile"), "worker: echo hello\n").unwrap();
    fs::write(paths.env_root.join("myapp").join("SCALING"), "worker=two\n").unwrap();
    let running = paths.workers_enabled.join("myapp@worker.1.toml");
    fs::write(&running, "[worker]\n").unwrap();
    let processes = HashMap::from([("worker".to_string(), "echo hello".to_string())]);

    let deployed = create_workers_generic(
        "myapp",
        &app_path,
        Some(&processes),
        &HashMap::new(),
        &paths,
        None,
    );

    let err = deployed.expect_err("a copy count that is not a number must fail the deploy");
    assert!(
        format!("{err:#}").contains("worker=two"),
        "the error names the bad line: {err:#}"
    );
    assert!(
        running.exists(),
        "the running worker's config must stay enabled when the deploy is refused"
    );
}
