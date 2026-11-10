use super::*;

#[test]
fn root_init_summary_checks_the_deploy_user_not_root() {
    let scope = InstallScope::System {
        user: "riku".to_string(),
    };
    assert_eq!(scope.systemctl_args(), ["is-active", "riku"]);
}

#[test]
fn guidance_has_no_placeholder_or_procfile_requirement() {
    let guide = first_app_guide("riku").join("\n");
    assert!(
        guide.contains("rk server add <name> riku@<host> --admin <login>@<host>"),
        "the guide should name the deploy user in rk server add:\n{guide}"
    );
    assert!(!guide.contains("your-server"), "fake host in:\n{guide}");
    assert!(
        !guide.contains("> Procfile"),
        "a Procfile is not required:\n{guide}"
    );
}
