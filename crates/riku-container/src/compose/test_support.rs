//! Shared fixtures for the compose tests: a throwaway app directory with a
//! chosen name, a compose file written into it, and `read_project` run on it;
//! and a single service parsed on its own.

use super::{
    default_compose_file, find_compose_file, parse_service, read_project, ComposeProject,
    ServiceContext, ServiceDef, Value,
};
use std::collections::{BTreeMap, BTreeSet};
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

/// One service parsed on its own, in a throwaway app directory that is also
/// the compose directory. `pgdata` is the only declared volume.
pub(super) struct ServiceFixture {
    pub app: TempDir,
    pub vars: BTreeMap<String, String>,
    pub volumes: BTreeSet<String>,
}

impl ServiceFixture {
    pub fn new() -> ServiceFixture {
        ServiceFixture {
            app: TempDir::new().unwrap(),
            vars: BTreeMap::new(),
            volumes: ["pgdata".to_string()].into(),
        }
    }

    pub fn parse(&self, name: &str, value: Value) -> anyhow::Result<ServiceDef> {
        let ctx = ServiceContext {
            app_path: self.app.path(),
            compose_dir: self.app.path(),
            vars: &self.vars,
            declared_volumes: &self.volumes,
        };
        parse_service(&ctx, name, &value)
    }

    pub fn write(&self, relative: &str, text: &str) {
        fs::write(self.app.path().join(relative), text).unwrap();
    }
}

/// The full error chain of a parse that must fail.
pub(super) fn service_error(result: anyhow::Result<ServiceDef>) -> String {
    format!("{:#}", result.expect_err("parse succeeded"))
}

/// An app called `shop` whose `compose.yaml` holds `text`, read with no env.
pub(super) fn project_of(text: &str) -> anyhow::Result<ComposeProject> {
    let app = TestApp::new("shop");
    app.write("compose.yaml", text);
    app.read(None, &[])
}
