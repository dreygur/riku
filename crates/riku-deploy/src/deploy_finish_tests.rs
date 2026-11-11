use std::collections::HashMap;

use super::live_at;
use crate::config::RikuPaths;

/// A Procfile with only `worker:` serves no HTTP, so the push summary must
/// not show a URL for it.
#[test]
fn a_worker_only_app_has_no_url() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.workers_enabled).unwrap();
    let config = crate::supervisor::config::create_worker_config(
        "shop",
        "worker",
        "run",
        1,
        HashMap::new(),
        "/srv/app",
        "/srv/log",
    );
    std::fs::write(
        paths.workers_enabled.join("shop@worker.1.toml"),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();

    let lines = live_at("shop", &HashMap::new(), &paths, false);

    assert!(lines.is_empty(), "no URL lines expected: {lines:?}");
}
