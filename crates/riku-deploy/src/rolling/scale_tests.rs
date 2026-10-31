use super::super::mock_copies::{calls, mock_routes, mock_runtime, CallLog};
use super::*;

#[test]
fn scale_to_adds_a_copy_without_restarting_others() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2, 3]);
    let mut routes = mock_routes(&log, None);

    scale_to(&mut runtime, &mut routes, 4).unwrap();

    assert_eq!(calls(&log), ["start 4", "healthy 4", "publish 4"]);
}

#[test]
fn scale_to_removes_the_highest_index_first() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2, 3]);
    let mut routes = mock_routes(&log, None);

    scale_to(&mut runtime, &mut routes, 1).unwrap();

    assert_eq!(calls(&log), ["publish 2", "stop 3", "publish 1", "stop 2"]);
}

#[test]
fn scale_to_zero_is_refused_untouched() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2]);
    let mut routes = mock_routes(&log, None);

    assert!(scale_to(&mut runtime, &mut routes, 0).is_err());
    assert!(calls(&log).is_empty(), "touched: {:?}", calls(&log));
}
