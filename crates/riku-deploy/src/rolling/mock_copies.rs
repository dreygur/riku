//! Test doubles for the rolling tests. Both record into one shared call log
//! so tests assert on the order across runtime and router.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::{anyhow, Result};
use riku_container::types::CopyIndex;
use riku_util::backend::Backend;

use super::replace::{CopyHandle, CopyRuntime, LiveCopy, RouteSink};

pub type CallLog = Rc<RefCell<Vec<String>>>;

#[derive(Clone, Copy, PartialEq)]
pub enum NewCopy {
    Healthy,
    Unhealthy,
}

pub struct MockRuntime {
    log: CallLog,
    pub running: Vec<LiveCopy>,
    pub new_copy: NewCopy,
}

pub struct MockRoutes {
    log: CallLog,
    publishes: usize,
    fail_publish: Option<usize>,
    /// Makes every publish from this 1-based call on fail, as a router that
    /// went down does.
    pub down_from: Option<usize>,
    /// Makes `check` fail, as for a snippet nginx refuses.
    pub refuse_check: bool,
    /// Every list handed to `publish`, in order.
    pub published: Vec<Vec<Backend>>,
}

/// The backend `copy_at(n)` reports: loopback port 9000 + n.
pub fn backend_of(n: u32) -> Backend {
    Backend::loopback(9000 + n as u16)
}

fn copy_at(n: u32) -> LiveCopy {
    LiveCopy {
        copy: CopyIndex::new(n).unwrap(),
        backend: Some(backend_of(n)),
        handle: CopyHandle::Process(format!("web-{n}")),
    }
}

pub fn mock_runtime(log: &CallLog, running: &[u32]) -> MockRuntime {
    MockRuntime {
        log: log.clone(),
        running: running.iter().map(|n| copy_at(*n)).collect(),
        new_copy: NewCopy::Healthy,
    }
}

/// `fail_publish` is the 1-based publish call that returns an error.
pub fn mock_routes(log: &CallLog, fail_publish: Option<usize>) -> MockRoutes {
    MockRoutes {
        log: log.clone(),
        publishes: 0,
        fail_publish,
        down_from: None,
        refuse_check: false,
        published: Vec::new(),
    }
}

pub fn calls(log: &CallLog) -> Vec<String> {
    log.borrow().clone()
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
        self.published.push(backends.to_vec());
        self.log
            .borrow_mut()
            .push(format!("publish {}", backends.len()));
        let down = self.down_from.is_some_and(|from| self.publishes >= from);
        if down || self.fail_publish == Some(self.publishes) {
            return Err(anyhow!("reload not confirmed"));
        }
        Ok(())
    }

    fn check(&mut self) -> Result<()> {
        if self.refuse_check {
            return Err(anyhow!("snippet refused"));
        }
        Ok(())
    }
}
