use anyhow::anyhow;

use super::*;
use crate::doctor::Status;

#[test]
fn inactive_unit_fails_with_the_fix_command() {
    let check = judge(
        Some(false),
        Ok(Reply::Status {
            version: "1".into(),
        }),
    );
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(check.detail.contains(FIX), "{}", check.detail);
}

#[test]
fn unanswered_socket_fails_even_without_systemd() {
    let check = judge(None, Err(anyhow!("cannot reach it")));
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(check.detail.contains(FIX), "{}", check.detail);
}

#[test]
fn status_reply_from_an_active_unit_is_ok() {
    let check = judge(
        Some(true),
        Ok(Reply::Status {
            version: "1".into(),
        }),
    );
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
}
