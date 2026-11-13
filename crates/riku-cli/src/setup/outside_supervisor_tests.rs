use super::*;

/// A supervisor a deploy started with nohup keeps the pid file lock, so the
/// unit's own would loop on the lock while it runs. Any binary, and whether
/// the unit runs (a main pid) or not (none), it must be stopped; only the
/// unit's own main process is left for root to manage.
#[test]
fn any_locked_supervisor_outside_the_units_is_replaced() {
    let units = |own, other| MainPids { own, other };
    assert_eq!(
        holder(units(None, None), 51),
        Holder::Outside(51),
        "a stopped unit must not be left to loop on the lock"
    );
    assert_eq!(
        holder(units(Some(77), None), 51),
        Holder::Outside(51),
        "a running unit whose supervisor lost the lock"
    );
    assert_eq!(
        holder(units(Some(51), None), 51),
        Holder::OwnUnit,
        "the unit's own supervisor is root's to restart"
    );
}

/// A plain init on a host whose system unit runs the supervisor (same
/// deploy user, same lock) must not SIGTERM it: the system unit restarts it
/// and fights the new user unit.
#[test]
fn the_other_scopes_supervisor_is_never_replaced() {
    let units = MainPids {
        own: None,
        other: Some(51),
    };

    assert_eq!(holder(units, 51), Holder::OtherUnit);
    let message = other_unit_error(UnitScope::User).to_string();
    assert!(message.contains("sudo riku init"), "{message}");
}
