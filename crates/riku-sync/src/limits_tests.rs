use super::*;
use crate::session_kit::{changed_source, error_reported_to_client, honest_client, refused_under};
use crate::testkit::fixture;
use crate::{receive, scan};
use std::io::Cursor;

/// `changed_source` needs `dir/inner.txt` (7 bytes) and `new.txt` (1 byte)
/// from the fixture's live tree.
const NEEDED: u64 = 8;

fn limits(max_needed_bytes: u64, free_margin_bytes: u64) -> ReceiveLimits {
    ReceiveLimits {
        max_needed_bytes,
        free_margin_bytes,
    }
}

#[test]
fn sync_over_the_size_limit_is_refused_before_the_plan() {
    let f = fixture();
    let src = changed_source();
    let (mut input, bundle) = honest_client(src.path(), &f.live);
    input.extend(bundle);

    let (err, reply) = refused_under(&f, &limits(NEEDED - 1, 0), Cursor::new(input));

    assert!(err.contains("8 bytes"), "server error was: {err}");
    assert_eq!(reply.lines().count(), 1, "a plan was sent: {reply}");
    assert_eq!(
        error_reported_to_client(&reply),
        "{\"error\":\"the sync is larger than this server's limit of 7 bytes\"}"
    );
}

#[test]
fn sync_within_the_size_limit_is_applied() {
    let f = fixture();
    let src = changed_source();
    let (mut input, bundle) = honest_client(src.path(), &f.live);
    input.extend(bundle);
    let mut reply = Vec::new();

    let served = receive(&f.live, &limits(NEEDED, 0), Cursor::new(input), &mut reply);

    assert!(served.is_ok(), "refused: {served:?}");
    assert_eq!(scan(&f.live).unwrap(), scan(src.path()).unwrap());
}

#[test]
fn sync_that_would_eat_the_free_space_margin_is_refused() {
    let f = fixture();
    let src = changed_source();
    let (mut input, bundle) = honest_client(src.path(), &f.live);
    input.extend(bundle);

    let (err, reply) = refused_under(&f, &limits(NEEDED, u64::MAX / 2), Cursor::new(input));

    assert!(err.contains("free"), "server error was: {err}");
    assert_eq!(
        error_reported_to_client(&reply),
        "{\"error\":\"the server does not have enough free disk space for this sync\"}"
    );
}
