use super::event_data;

#[test]
fn a_stamped_line_sends_its_own_time() {
    assert_eq!(
        event_data("web.1", "2026-11-12T08:14:03Z listening on :37139"),
        "web.1\t2026-11-12T08:14:03Z\tlistening on :37139"
    );
    assert_eq!(
        event_data("deploy", "[2026-11-12T14:14:03+0600] Syncing repo to HEAD"),
        "deploy\t2026-11-12T08:14:03Z\tSyncing repo to HEAD"
    );
}

/// No stamp, no time: the panel then shows none rather than when the
/// browser happened to receive the line.
#[test]
fn a_line_without_a_stamp_sends_an_empty_time() {
    assert_eq!(
        event_data("deploy", "[WARN] cache\tmissing"),
        "deploy\t\t[WARN] cache\tmissing"
    );
}
