use super::*;

#[test]
fn root_init_summary_checks_the_deploy_user_not_root() {
    let scope = InstallScope::System {
        user: "riku".to_string(),
        home: PathBuf::from("/home/riku"),
    };

    assert_eq!(scope.binary(), PathBuf::from("/home/riku/.local/bin/riku"));
    assert_eq!(scope.systemctl_args(), ["is-active", "riku"]);
    assert_eq!(
        remote_line(scope.user()),
        "   git remote add riku riku@your-server:myapp"
    );
}
