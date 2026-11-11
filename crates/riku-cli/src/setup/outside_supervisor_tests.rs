use super::*;

/// A supervisor a deploy started with nohup keeps the pid file lock, so the
/// unit's own would loop on the lock while it runs. Any binary, and whether
/// the unit runs (a main pid) or not (none), it must be stopped; only the
/// unit's own main process is left for root to manage.
#[test]
fn any_locked_supervisor_outside_the_unit_is_replaced() {
    assert_eq!(
        to_replace(None, Some(51)),
        Some(51),
        "a stopped unit must not be left to loop on the lock"
    );
    assert_eq!(
        to_replace(Some(77), Some(51)),
        Some(51),
        "a running unit whose supervisor lost the lock"
    );
    assert_eq!(
        to_replace(Some(51), Some(51)),
        None,
        "the unit's own supervisor is root's to restart"
    );
    assert_eq!(to_replace(None, None), None, "nothing holds the lock");
}
