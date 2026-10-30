//! Shared fixture for the project level tests: a throwaway app directory with
//! a chosen name, a compose file written into it, and `read_project` run on it.

use super::{default_compose_file, find_compose_file, read_project, ComposeProject};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

pub(super) struct TestApp {
    _tmp: TempDir,
    pub path: PathBuf,
}

impl TestApp {
    /// An empty app directory called `dir_name`, inside its own temp dir.
    pub fn new(dir_name: &str) -> TestApp {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(dir_name);
        fs::create_dir(&path).unwrap();
        TestApp { _tmp: tmp, path }
    }

    pub fn write(&self, relative: &str, text: &str) {
        let target = self.path.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, text).unwrap();
    }

    /// Read the project from `compose.yaml`, or from `compose_file` when given.
    pub fn read(
        &self,
        compose_file: Option<&str>,
        env: &[(&str, &str)],
    ) -> anyhow::Result<ComposeProject> {
        let compose = match compose_file {
            Some(name) => find_compose_file(&self.path, Some(name)).unwrap().unwrap(),
            None => default_compose_file(&self.path).expect("no compose.yaml was written"),
        };
        let env: BTreeMap<String, String> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        read_project(&self.path, &compose, &env)
    }
}

/// An app called `shop` whose `compose.yaml` holds `text`, read with no env.
pub(super) fn project_of(text: &str) -> anyhow::Result<ComposeProject> {
    let app = TestApp::new("shop");
    app.write("compose.yaml", text);
    app.read(None, &[])
}
