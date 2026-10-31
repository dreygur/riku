use std::cell::RefCell;
use std::rc::Rc;

use anyhow::anyhow;

use super::super::scale::scale_to;
use super::*;

type CallLog = Rc<RefCell<Vec<String>>>;

#[derive(Clone, Copy, PartialEq)]
enum NewCopy {
    Healthy,
    Unhealthy,
}

struct MockRuntime {
    log: CallLog,
    running: Vec<LiveCopy>,
    new_copy: NewCopy,
}

struct MockRoutes {
    log: CallLog,
    publishes: usize,
    fail_publish: Option<usize>,
}

fn copy_at(n: u32) -> LiveCopy {
    LiveCopy {
        copy: CopyIndex::new(n).unwrap(),
        backend: Some(Backend::loopback(9000 + n as u16)),
        handle: CopyHandle::Process(format!("web-{n}")),
    }
}

fn mock_runtime(log: &CallLog, running: &[u32]) -> MockRuntime {
    MockRuntime {
        log: log.clone(),
        running: running.iter().map(|n| copy_at(*n)).collect(),
        new_copy: NewCopy::Healthy,
    }
}

fn mock_routes(log: &CallLog, fail_publish: Option<usize>) -> MockRoutes {
    MockRoutes {
        log: log.clone(),
        publishes: 0,
        fail_publish,
    }
}

impl CopyRuntime for MockRuntime {
    fn live(&self) -> Result<Vec<LiveCopy>> {
        Ok(self.running.clone())
    }

    fn start(&mut self, copy: CopyIndex) -> Result<LiveCopy> {
        self.log.borrow_mut().push(format!("start {copy}"));
        let started = copy_at(copy.get());
        self.running.push(started.clone());
        Ok(started)
    }

    fn wait_healthy(&self, copy: &LiveCopy) -> Result<()> {
        self.log.borrow_mut().push(format!("healthy {}", copy.copy));
        match self.new_copy {
            NewCopy::Healthy => Ok(()),
            NewCopy::Unhealthy => Err(anyhow!("timed out")),
        }
    }

    fn stop(&mut self, copy: &LiveCopy) -> Result<()> {
        self.log.borrow_mut().push(format!("stop {}", copy.copy));
        self.running.retain(|c| c.copy != copy.copy);
        Ok(())
    }

    fn discard(&mut self, copy: &LiveCopy) {
        self.log.borrow_mut().push(format!("discard {}", copy.copy));
        self.running.retain(|c| c.copy != copy.copy);
    }
}

impl RouteSink for MockRoutes {
    fn publish(&mut self, backends: &[Backend]) -> Result<()> {
        self.publishes += 1;
        self.log
            .borrow_mut()
            .push(format!("publish {}", backends.len()));
        if self.fail_publish == Some(self.publishes) {
            return Err(anyhow!("reload not confirmed"));
        }
        Ok(())
    }
}

fn calls(log: &CallLog) -> Vec<String> {
    log.borrow().clone()
}

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
fn single_routed_copy_stops_without_an_empty_publish() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1]);
    let mut routes = mock_routes(&log, None);

    replace_all(&mut runtime, &mut routes, ReplaceOrder::StopFirst, 1).unwrap();

    assert_eq!(calls(&log), ["stop 1", "start 1", "healthy 1", "publish 1"]);
}

#[test]
fn scale_to_zero_is_refused_untouched() {
    let log = CallLog::default();
    let mut runtime = mock_runtime(&log, &[1, 2]);
    let mut routes = mock_routes(&log, None);

    assert!(scale_to(&mut runtime, &mut routes, 0).is_err());
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
