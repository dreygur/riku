use std::fs;

use super::*;

fn shop() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    (tmp, paths)
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn secret_key_needs_a_token_bound_to_the_exact_values() {
    let (tmp, paths) = shop();

    let asked = cmd_agent_config_set(&paths, tmp.path(), "shop", &args(&["DATABASE_URL=a"]), None);
    assert!(
        asked.confirmation_required,
        "secret key applied without asking"
    );
    let token = asked.confirm_token.expect("no confirm token issued");

    let swapped = cmd_agent_config_set(
        &paths,
        tmp.path(),
        "shop",
        &args(&["DATABASE_URL=evil"]),
        Some(&token),
    );
    let code = swapped.error.map(|e| e.code);
    assert_eq!(
        code.as_deref(),
        Some("INVALID_TOKEN"),
        "token reused for other values"
    );
    assert!(
        !paths.env_root.join("shop/ENV").exists(),
        "ENV written anyway"
    );
}

#[test]
fn plain_key_is_saved_and_a_failed_redeploy_says_so() {
    let (tmp, paths) = shop();

    let resp = cmd_agent_config_set(&paths, tmp.path(), "shop", &args(&["DEBUG=1"]), None);

    assert!(
        !resp.confirmation_required,
        "plain key asked for confirmation"
    );
    let env = fs::read_to_string(paths.env_root.join("shop/ENV")).unwrap();
    assert!(env.contains("DEBUG=1"), "ENV is {env:?}");
    let error = resp
        .error
        .expect("a test app has no runtime, so redeploy fails");
    assert_eq!(error.code, "DEPLOY_FAILED");
    assert!(error.message.contains("saved"), "{}", error.message);
}

#[test]
fn riku_setting_is_refused_before_anything_is_written() {
    let (tmp, paths) = shop();

    let resp = cmd_agent_config_set(&paths, tmp.path(), "shop", &args(&["RIKU_DOMAINS=x"]), None);

    let code = resp.error.map(|e| e.code);
    assert_eq!(code.as_deref(), Some("INVALID_PARAMETERS"));
    assert!(!paths.env_root.join("shop/ENV").exists());
}

#[test]
fn one_argument_cannot_smuggle_a_second_key() {
    let (tmp, paths) = shop();

    cmd_agent_config_set(
        &paths,
        tmp.path(),
        "shop",
        &args(&["DEBUG=1 API_KEY=x"]),
        None,
    );

    let mut env = HashMap::new();
    parse_settings(&paths.env_root.join("shop/ENV"), &mut env).unwrap();
    assert_eq!(env.get("DEBUG").map(String::as_str), Some("1 API_KEY=x"));
    assert!(!env.contains_key("API_KEY"), "API_KEY set without a token");
}

#[test]
fn unsetting_a_secret_key_also_needs_a_token() {
    let (tmp, paths) = shop();
    fs::write(paths.env_root.join("shop/ENV"), "API_KEY=x\n").unwrap();

    let resp = cmd_agent_config_unset(&paths, tmp.path(), "shop", &args(&["API_KEY"]), None);

    assert!(
        resp.confirmation_required,
        "secret key removed without asking"
    );
}
