use super::*;

#[test]
fn service_count_parses_and_trims_valid_setting() {
    let (service, count) = match service_count("web=3") {
        Ok(res) => res,
        Err(e) => panic!("expected 'web=3' to parse: {e}"),
    };
    assert_eq!(
        service.as_str(),
        "web",
        "expected service 'web', got {}",
        service.as_str()
    );
    assert_eq!(count, 3, "expected count 3, got {count}");

    let (service, count) = match service_count(" web = 3 ") {
        Ok(res) => res,
        Err(e) => panic!("expected ' web = 3 ' to parse: {e}"),
    };
    assert_eq!(
        service.as_str(),
        "web",
        "expected service 'web', got {}",
        service.as_str()
    );
    assert_eq!(count, 3, "expected count 3, got {count}");
}

#[test]
fn service_count_rejects_invalid_settings_naming_the_setting() {
    for setting in ["web", "web=-1", "=2", "web=0"] {
        let err = match service_count(setting) {
            Ok(v) => panic!("expected '{setting}' to fail, got {v:?}"),
            Err(e) => e,
        };
        let msg = format!("{err:#}");
        assert!(
            msg.contains(setting),
            "error for '{setting}' should contain '{setting}', got: {msg}"
        );
    }
}

#[test]
fn scale_procfile_refuses_a_bad_count_before_any_deploy() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let env_dir = paths.env_root.join("shop");
    std::fs::create_dir_all(&env_dir).unwrap();
    std::fs::write(env_dir.join("SCALING"), "web=1\n").unwrap();

    for setting in ["web=-1", "web=two", "web"] {
        let settings = vec!["worker=2".to_string(), setting.to_string()];
        let err = match scale_procfile(&paths, "shop", &settings) {
            Ok(()) => panic!("'{setting}' must fail the scale"),
            Err(e) => format!("{e:#}"),
        };
        assert!(err.contains(setting), "error must name '{setting}': {err}");
        assert_eq!(
            std::fs::read_to_string(env_dir.join("SCALING")).unwrap(),
            "web=1\n",
            "'{setting}' changed SCALING"
        );
    }
}

#[test]
fn scale_containers_fails_on_malformed_setting_before_scaling() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let app = match AppName::try_from("shop") {
        Ok(app) => app,
        Err(e) => panic!("failed to create AppName: {e}"),
    };
    let settings = vec!["web=2".to_string(), "bad".to_string()];

    let err = match scale_containers(&paths, &app, &settings) {
        Ok(()) => panic!("expected scale_containers to fail on malformed setting"),
        Err(e) => e,
    };
    let msg = format!("{err:#}");
    assert!(
        msg.contains("malformed setting 'bad'"),
        "expected malformed setting error for 'bad', got: {msg}"
    );
}
