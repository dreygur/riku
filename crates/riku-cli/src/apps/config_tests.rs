use std::fs;

use super::*;

fn app_with_env(tmp: &std::path::Path) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    fs::write(paths.env_root.join("shop").join("ENV"), "PORT=5000\n").unwrap();
    paths
}

#[test]
fn malformed_setting_fails_and_keeps_env() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app_with_env(tmp.path());

    let result = cmd_config_set(&paths, "shop", &["NOEQUALS".to_string()]);
    assert!(
        result.is_err(),
        "config set reported success for a malformed setting"
    );

    let env = fs::read_to_string(paths.env_root.join("shop").join("ENV")).unwrap();
    assert_eq!(env, "PORT=5000\n");
}

#[test]
fn each_argument_is_one_setting_so_values_keep_their_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app_with_env(tmp.path());

    let env = config_after_set(&paths, "shop", &["MSG=hello world".to_string()]).unwrap();

    assert_eq!(env.get("MSG").map(String::as_str), Some("hello world"));
}

#[test]
fn isolation_root_is_refused_as_a_plain_env_key() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app_with_env(tmp.path());

    let result = config_after_set(&paths, "shop", &["RIKU_ISOLATION_ROOT=/".to_string()]);

    match result {
        Ok(_) => panic!("config set accepted RIKU_ISOLATION_ROOT"),
        Err(e) => assert!(
            e.to_string().contains("limits.isolation_root"),
            "should point at the server setting: {e}"
        ),
    }
}

#[test]
fn newline_in_value_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = app_with_env(tmp.path());

    let result = cmd_config_set(&paths, "shop", &["\"A=x\ny\"".to_string()]);
    assert!(
        result.is_err(),
        "config set reported success for a value with a newline"
    );
}
