use std::fs;

use super::*;

fn shop() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    fs::write(
        paths.env_root.join("shop/ENV"),
        "DATABASE_URL=postgres://u:pw@db/shop\nDEBUG=1\n",
    )
    .unwrap();
    (tmp, paths)
}

#[test]
fn readonly_agent_sees_secret_values_masked() {
    let (_tmp, paths) = shop();

    let shown = cmd_agent_config_show(&paths, &AgentScope::Readonly, "shop");
    let got = cmd_agent_config_get(&paths, &AgentScope::Readonly, "shop", "DATABASE_URL");

    let config = &shown.data.expect("show returned no data")["config"];
    assert_eq!(config["DATABASE_URL"], "***", "show leaked the secret");
    assert_eq!(config["DEBUG"], "1", "plain value was masked");
    let value = &got.data.expect("get returned no data")["value"];
    assert_eq!(value, "***", "get leaked the secret");
}

#[test]
fn staging_agent_sees_secret_values() {
    let (_tmp, paths) = shop();

    let got = cmd_agent_config_get(&paths, &AgentScope::Staging, "shop", "DATABASE_URL");

    let value = &got.data.expect("get returned no data")["value"];
    assert_eq!(value, "postgres://u:pw@db/shop");
}
