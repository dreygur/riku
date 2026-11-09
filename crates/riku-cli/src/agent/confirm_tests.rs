use super::*;

#[test]
fn issued_token_works_once() {
    let tmp = tempfile::tempdir().unwrap();
    let token = issue_at(tmp.path(), "stop", "shop", 100).unwrap();

    assert_eq!(redeem_at(tmp.path(), "stop", "shop", &token, 101), Ok(()));
    assert_eq!(
        redeem_at(tmp.path(), "stop", "shop", &token, 102),
        Err(Refusal::Unknown),
        "a spent token must not work again"
    );
}

#[test]
fn refuses_token_for_another_app_or_action() {
    let tmp = tempfile::tempdir().unwrap();
    let token = issue_at(tmp.path(), "stop", "shop", 100).unwrap();
    assert_eq!(
        redeem_at(tmp.path(), "destroy", "shop", &token, 101),
        Err(Refusal::Mismatch)
    );

    let token = issue_at(tmp.path(), "stop", "shop", 100).unwrap();
    assert_eq!(
        redeem_at(tmp.path(), "stop", "blog", &token, 101),
        Err(Refusal::Mismatch)
    );
}

#[test]
fn refuses_expired_token() {
    let tmp = tempfile::tempdir().unwrap();
    let token = issue_at(tmp.path(), "stop", "shop", 100).unwrap();
    let late = 100 + TOKEN_TTL_SECS + 1;
    assert_eq!(
        redeem_at(tmp.path(), "stop", "shop", &token, late),
        Err(Refusal::Expired)
    );
}

#[test]
fn refuses_path_shaped_token_before_touching_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("victim");
    std::fs::write(&outside, "stop\nshop\n999999999999\n").unwrap();

    let token = format!("../{}", "a".repeat(TOKEN_HEX_LEN - 3));
    assert_eq!(
        redeem(tmp.path(), "stop", "shop", &token),
        Err(Refusal::Malformed)
    );
    assert!(outside.exists(), "a refused token must not delete files");
}
