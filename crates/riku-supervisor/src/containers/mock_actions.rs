//! Mock control actions for supervisor container tests.

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use riku_config::RikuPaths;

use crate::health::actions::{ControlActions, RestartOutcome};

fn unexpected_call() -> anyhow::Error {
    anyhow!("unexpected control action in test")
}

/// Test double recording restart calls and returning a settable outcome.
pub(crate) struct MockActions {
    pub(crate) outcome: Mutex<RestartOutcome>,
    pub(crate) calls: Mutex<Vec<(String, String, u32)>>,
}

/// Create a new mock actions handle initialized with `outcome`.
pub(crate) fn mock_actions(outcome: RestartOutcome) -> Arc<MockActions> {
    Arc::new(MockActions {
        outcome: Mutex::new(outcome),
        calls: Mutex::new(Vec::new()),
    })
}

impl MockActions {
    pub(crate) fn call_count(&self) -> usize {
        match self.calls.lock() {
            Ok(calls) => calls.len(),
            Err(_) => 0,
        }
    }
}

impl ControlActions for MockActions {
    fn create_app(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(unexpected_call())
    }

    fn deploy(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(unexpected_call())
    }

    fn restart(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(unexpected_call())
    }

    fn stop(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(unexpected_call())
    }

    fn destroy(&self, _: &RikuPaths, _: &str) -> Result<()> {
        Err(unexpected_call())
    }

    fn install_plugins(&self, _: &RikuPaths, _: Option<Vec<String>>) -> Result<()> {
        Err(unexpected_call())
    }

    fn container_export(&self, _: &str, _: &Path, _: &Path) -> Result<()> {
        Err(unexpected_call())
    }

    fn list_client_plugins(&self) -> Result<Vec<String>> {
        Err(unexpected_call())
    }

    fn pull_service(&self, _: &RikuPaths, _: &str, _: &str) -> Result<()> {
        Err(unexpected_call())
    }

    fn restart_copy(
        &self,
        _: &RikuPaths,
        app: &str,
        service: &str,
        copy: u32,
    ) -> Result<RestartOutcome> {
        match self.calls.lock() {
            Ok(mut calls) => calls.push((app.to_string(), service.to_string(), copy)),
            Err(e) => return Err(anyhow!("calls lock poisoned: {e}")),
        }
        match self.outcome.lock() {
            Ok(outcome) => Ok(*outcome),
            Err(e) => Err(anyhow!("outcome lock poisoned: {e}")),
        }
    }
}
