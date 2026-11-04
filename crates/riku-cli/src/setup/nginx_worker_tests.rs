use super::*;

#[test]
fn worker_user_is_read_from_the_main_user_directive() {
    let conf = "# user nobody;\nuser_agent off;\nuser nginx nginx;\nworker_processes auto;\n";
    assert_eq!(configured_user(conf).as_deref(), Some("nginx"));
    assert_eq!(configured_user("worker_processes 1;\n"), None);
    assert!(!is_account_name("-o"), "an option is not an account name");
}
