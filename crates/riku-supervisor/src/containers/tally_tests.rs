use std::time::{Duration, Instant};

use riku_container::engine::ContainerId;

use super::RestartTally;

#[test]
fn deploy_resets_restart_count() {
    let mut tally = RestartTally::new();
    let name = "myapp-web-1";
    let now = Instant::now();

    let id1 =
        ContainerId::try_from("1111111111111111111111111111111111111111111111111111111111111111")
            .unwrap();
    let id2 =
        ContainerId::try_from("2222222222222222222222222222222222222222222222222222222222222222")
            .unwrap();
    let id3 =
        ContainerId::try_from("3333333333333333333333333333333333333333333333333333333333333333")
            .unwrap();
    let id4 =
        ContainerId::try_from("4444444444444444444444444444444444444444444444444444444444444444")
            .unwrap();

    tally.observe(name, &id1);
    assert_eq!(tally.count(name), 0, "initial count must be 0");

    tally.record_restart(name, now);
    assert_eq!(tally.count(name), 1, "first restart increments count to 1");

    tally.observe(name, &id2);
    assert_eq!(
        tally.count(name),
        1,
        "successor id observation must preserve restart count"
    );

    tally.record_restart(name, now);
    assert_eq!(tally.count(name), 2, "second restart increments count to 2");

    tally.observe(name, &id3);
    assert_eq!(
        tally.count(name),
        2,
        "successor id observation must preserve restart count"
    );

    tally.observe(name, &id4);
    assert_eq!(
        tally.count(name),
        0,
        "deploy observing new id without restart must reset count to 0"
    );
}

#[test]
fn backoff_gates_next_restart() {
    let mut tally = RestartTally::new();
    let name = "myapp-web-1";
    let id =
        ContainerId::try_from("1111111111111111111111111111111111111111111111111111111111111111")
            .unwrap();

    tally.observe(name, &id);
    let t = Instant::now();

    tally.record_restart(name, t);
    assert_eq!(tally.count(name), 1);

    assert!(
        !tally.due(name, t + Duration::from_secs(1)),
        "at t + 1 s, 2 s backoff must not be due"
    );
    assert!(
        tally.due(name, t + Duration::from_secs(2)),
        "at t + 2 s, 2 s backoff must be due"
    );
}
