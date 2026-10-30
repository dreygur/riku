use super::*;

#[test]
fn accepts_a_plain_login_name() {
    let name = AccountName::try_from("riku".to_string()).unwrap();
    assert_eq!(name.as_str(), "riku");
}

#[test]
fn rejects_names_useradd_would_misread_or_refuse() {
    for bad in [
        "",
        "-r",
        "Riku",
        "1riku",
        "ri ku",
        "../riku",
        &"r".repeat(33),
    ] {
        assert!(
            AccountName::try_from(bad.to_string()).is_err(),
            "{bad:?} should be rejected"
        );
    }
}
