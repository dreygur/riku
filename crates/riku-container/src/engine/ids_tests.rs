use super::ContainerId;

#[test]
fn accepts_short_and_full_length_hex_ids() {
    assert!(ContainerId::try_from("0123456789ab").is_ok());
    assert!(ContainerId::try_from("f".repeat(64).as_str()).is_ok());
}

#[test]
fn refuses_option_like_uppercase_short_and_long_ids() {
    for bad in ["--privileged", "0123456789AB", "abc", &"a".repeat(65), ""] {
        let err = ContainerId::try_from(bad).unwrap_err().to_string();
        assert!(err.contains("container id"), "{bad:?}: {err}");
    }
}
