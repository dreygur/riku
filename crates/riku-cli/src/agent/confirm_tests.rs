use super::*;

#[test]
fn issuing_removes_expired_records_and_keeps_live_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let old = issue_at(tmp.path(), "stop", "shop", 100).unwrap();
    let late = 100 + TOKEN_TTL_SECS + 1;
    let fresh = issue_at(tmp.path(), "stop", "shop", late - 1).unwrap();

    issue_at(tmp.path(), "stop", "shop", late).unwrap();

    assert!(
        !record_path(tmp.path(), &old).exists(),
        "expired record kept"
    );
    assert!(
        record_path(tmp.path(), &fresh).exists(),
        "live record removed"
    );
}

#[test]
fn bound_action_differs_when_any_word_changes() {
    let words = |list: &[&str]| list.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    let base = bound_action("run", &words(&["ls", "-la"]));
    assert_ne!(base, bound_action("run", &words(&["ls", "-lah"])));
    assert_ne!(
        base,
        bound_action("run", &words(&["ls -la"])),
        "word split ignored"
    );
}

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
