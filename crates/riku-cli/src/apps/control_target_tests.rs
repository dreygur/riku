use std::fs;

use super::*;

#[test]
fn invalid_service_on_container_app_carries_raw_value() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let app_dir = paths.app_root.join("shop");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(
        app_dir.join("compose.yaml"),
        "services:\n  web:\n    image: nginx:alpine\n",
    )
    .unwrap();

    let err = match control_target(&paths, "shop", Some("-web")) {
        Ok(_) => panic!("expected control_target to fail for invalid service '-web'"),
        Err(e) => e,
    };
    let msg = format!("{err:#}");
    assert!(
        msg.contains("-web"),
        "error message should contain raw value '-web', got: {msg}"
    );
}
