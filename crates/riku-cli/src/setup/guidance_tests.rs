use super::*;

#[test]
fn root_init_summary_checks_the_deploy_user_not_root() {
    let scope = InstallScope::System {
        user: "riku".to_string(),
    };
    assert_eq!(scope.systemctl_args(), ["is-active", "riku"]);
}

#[test]
fn init_summary_names_the_router_and_the_next_command() {
    let rows = finish_summary("riku", "caddy");
    let pairs: Vec<(&str, &str)> = rows.iter().map(|(k, v)| (*k, v.as_str())).collect();
    assert_eq!(
        display::render_summary(&pairs, false),
        "\n   Router   caddy\
         \n   Binary   /usr/local/bin/riku\
         \n   Next     rk server add <name> riku@<host> --admin <login>@<host>, then git push riku main\
         \n   Docs     https://dreygur.github.io/riku/"
    );
}
