use super::*;

fn check(words: &[&str]) -> Result<()> {
    let matches = crate::help::command("Forwarded commands:")
        .try_get_matches_from(words)
        .unwrap();
    check_remote_flags(&matches)
}

#[test]
fn rejects_local_only_flags_for_a_remote_dashboard_by_name() {
    let bind = check(&["riku", "-s", "x", "dashboard", "--bind", "0.0.0.0:80"]);
    assert!(
        bind.as_ref()
            .is_err_and(|e| e.to_string().contains("--bind")),
        "{bind:?}"
    );
    let token = check(&["riku", "-s", "x", "dashboard", "--token", "abc"]);
    assert!(
        token
            .as_ref()
            .is_err_and(|e| e.to_string().contains("--token")),
        "{token:?}"
    );
}

#[test]
fn accepts_a_remote_dashboard_without_local_flags() {
    let plain = check(&["riku", "-s", "x", "dashboard"]);
    assert!(plain.is_ok(), "{plain:?}");
}
