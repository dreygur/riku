use super::super::mock_copies::{backend_of, calls, mock_routes, mock_runtime, CallLog, NewCopy};
use super::*;

#[test]
fn failed_copy_leaves_old_serving() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2]);
    runtime.new_copy = NewCopy::Unhealthy;
    let mut routes = mock_routes(&log, None);

    let outcome = replace_all(&mut runtime, &mut routes, ReplaceOrder::Overlap, 2);

    assert!(outcome.is_err(), "an unhealthy copy must fail the deploy");
    assert_eq!(calls(&log), ["start 3", "healthy 3", "discard 3"]);
    assert_eq!(runtime.running.len(), 2, "old copies keep running");
}

#[test]
fn unconfirmed_reload_keeps_old_copy() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1]);
    let mut routes = mock_routes(&log, Some(2));

    let outcome = replace_all(&mut runtime, &mut routes, ReplaceOrder::Overlap, 1);

    assert!(
        outcome.is_err(),
        "an unconfirmed drain must fail the deploy"
    );
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("stop")),
        "old copy stopped without a confirmed drain: {:?}",
        calls(&log)
    );
}

#[test]
fn unconfirmed_add_keeps_new_copy_running() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1]);
    let mut routes = mock_routes(&log, Some(1));

    let outcome = replace_all(&mut runtime, &mut routes, ReplaceOrder::Overlap, 1);

    let message = format!("{:#}", outcome.unwrap_err());
    assert!(
        message.contains("copy 2 left running") && message.contains("next deploy"),
        "error must name the copy left running: {message}"
    );
    assert_eq!(calls(&log), ["start 2", "healthy 2", "publish 2"]);
}

#[test]
fn overlap_replaces_one_at_a_time() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2, 3]);
    let mut routes = mock_routes(&log, None);

    replace_all(&mut runtime, &mut routes, ReplaceOrder::Overlap, 3).unwrap();

    assert_eq!(
        calls(&log),
        [
            "start 4",
            "healthy 4",
            "publish 4",
            "publish 3",
            "stop 1",
            "start 1",
            "healthy 1",
            "publish 4",
            "publish 3",
            "stop 2",
            "start 2",
            "healthy 2",
            "publish 4",
            "publish 3",
            "stop 3",
        ]
    );
    assert_eq!(
        routes.published[0],
        [backend_of(1), backend_of(2), backend_of(3), backend_of(4)],
        "the new copy joins the old ones"
    );
    assert_eq!(
        routes.published[1],
        [backend_of(2), backend_of(3), backend_of(4)],
        "the drained copy leaves"
    );
}

#[test]
fn stop_first_never_overlaps_and_routes_to_the_new_copy() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2]);
    let mut routes = mock_routes(&log, None);

    replace_all(&mut runtime, &mut routes, ReplaceOrder::StopFirst, 2).unwrap();

    assert_eq!(
        calls(&log),
        [
            "publish 1",
            "stop 1",
            "start 1",
            "healthy 1",
            "publish 2",
            "publish 1",
            "stop 2",
            "start 2",
            "healthy 2",
            "publish 2",
        ]
    );
    assert_eq!(
        routes.published[1],
        [backend_of(2), backend_of(1)],
        "the replacement of copy 1 is published next to copy 2"
    );
}

#[test]
fn stop_first_failure_says_the_service_is_down() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1]);
    runtime.new_copy = NewCopy::Unhealthy;
    let mut routes = mock_routes(&log, None);

    let outcome = replace_all(&mut runtime, &mut routes, ReplaceOrder::StopFirst, 1);

    let message = format!("{:#}", outcome.unwrap_err());
    assert!(message.contains("service is down"), "got: {message}");
    assert_eq!(calls(&log), ["stop 1", "start 1", "healthy 1", "discard 1"]);
}

#[test]
fn stop_first_failure_counts_the_copies_still_serving() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2]);
    runtime.new_copy = NewCopy::Unhealthy;
    let mut routes = mock_routes(&log, None);

    let outcome = replace_all(&mut runtime, &mut routes, ReplaceOrder::StopFirst, 2);

    let message = format!("{:#}", outcome.unwrap_err());
    assert!(
        message.contains("copy 1 stopped") && message.contains("still serving: 1"),
        "got: {message}"
    );
    assert!(!message.contains("service is down"), "got: {message}");
}

#[test]
fn single_routed_copy_stops_without_an_empty_publish() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1]);
    let mut routes = mock_routes(&log, None);

    replace_all(&mut runtime, &mut routes, ReplaceOrder::StopFirst, 1).unwrap();

    assert_eq!(calls(&log), ["stop 1", "start 1", "healthy 1", "publish 1"]);
    assert_eq!(routes.published, [[backend_of(1)]]);
}

#[test]
fn replacing_with_zero_copies_is_refused_untouched() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2]);
    let mut routes = mock_routes(&log, None);

    let outcome = replace_all(&mut runtime, &mut routes, ReplaceOrder::Overlap, 0);

    assert!(outcome.is_err(), "want 0 must be refused");
    assert!(calls(&log).is_empty(), "touched: {:?}", calls(&log));
}

#[test]
fn scale_down_drains_before_stop() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2, 3]);
    let mut routes = mock_routes(&log, None);

    replace_all(&mut runtime, &mut routes, ReplaceOrder::Overlap, 1).unwrap();

    assert_eq!(
        calls(&log),
        [
            "start 4",
            "healthy 4",
            "publish 4",
            "publish 3",
            "stop 1",
            "publish 2",
            "stop 3",
            "publish 1",
            "stop 2",
        ]
    );
}

#[test]
fn scale_up_publishes_each_added_copy() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1]);
    let mut routes = mock_routes(&log, None);

    replace_all(&mut runtime, &mut routes, ReplaceOrder::Overlap, 3).unwrap();

    assert_eq!(
        calls(&log),
        [
            "start 2",
            "healthy 2",
            "publish 2",
            "publish 1",
            "stop 1",
            "start 1",
            "healthy 1",
            "publish 2",
            "start 3",
            "healthy 3",
            "publish 3",
        ]
    );
}
