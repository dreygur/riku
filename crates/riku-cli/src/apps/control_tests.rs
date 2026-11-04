use super::*;

#[test]
fn update_fails_and_names_the_installer() {
    let err = cmd_update().expect_err("update must not report success while unimplemented");
    let text = err.to_string();
    assert!(
        text.contains(INSTALL_SCRIPT_URL),
        "error lacks installer URL: {text}"
    );
}
