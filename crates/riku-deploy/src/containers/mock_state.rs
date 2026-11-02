//! A mock engine that remembers the containers it ran, for tests of whole
//! container deploys, plus a temp riku root routed through a mock Caddy.
//!
//! State lives in `<root>/state`: per container its name, labels and state
//! JSON; per volume its `vol-<name>` labels file; per network its `net-<name>`
//! marker file. Files a test creates there change how new containers behave:
//! `exits-<service>` makes them exit at once with code 1, `slow-<service>`
//! makes their health report `starting` for the first two inspects, and
//! `release-exits` makes `wait` report exit code 1 instead of 0. The
//! first inspect that reports a container running and healthy appends
//! `ready <name>` to the argv log, so tests can order it against runs.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::context::saved_env;
use super::{deploy_containers, AppName};
use crate::config::RikuPaths;
use crate::mock_engine::{logged, mock_engine};

/// A storage service `db` with a health check, and a routed `web` that
/// waits for `db` to be healthy.
pub const DB_AND_WEB: &str = "
services:
  db:
    image: postgres:16
    volumes: [\"data:/var/lib/postgresql/data\"]
    healthcheck:
      test: [\"CMD\", \"true\"]
  web:
    image: nginx:alpine
    ports: [\"80\"]
    depends_on:
      db:
        condition: service_healthy
    healthcheck:
      test: [\"CMD\", \"true\"]
volumes:
  data:
";

const BEHAVIOUR: &str = include_str!("mock_state.sh");

/// A temp riku server: mock engine, Caddy as the router (reload always
/// confirms), and the argv log and state directory of the engine.
pub struct MockServer {
    pub paths: RikuPaths,
    pub state: PathBuf,
    pub log: PathBuf,
}

impl MockServer {
    /// Engine command lines and `ready` markers, in order.
    pub fn calls(&self) -> Vec<String> {
        logged(&self.log)
    }

    /// Names of the containers the engine holds now.
    pub fn containers(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.state)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "name"))
            .map(|path| fs::read_to_string(path).unwrap().trim().to_string())
            .collect();
        names.sort();
        names
    }

    /// Mark new containers of `service` with a state file such as `exits`.
    pub fn mark(&self, flag: &str, service: &str) {
        fs::write(self.state.join(format!("{flag}-{service}")), "").unwrap();
    }

    /// The Caddy site file of `app`, empty when there is none.
    pub fn site(&self, app: &str) -> String {
        fs::read_to_string(self.paths.caddy_root.join(format!("{app}.caddy"))).unwrap_or_default()
    }
}

fn executable(path: &Path, script: &str) {
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A server under `root` with app `app` checked out with `compose` as its
/// compose file and `env` as its ENV file.
pub fn mock_server(root: &Path, app: &str, compose: &str, env: &str) -> MockServer {
    let state = root.join("state");
    fs::create_dir_all(&state).unwrap();
    let log = root.join("argv.log");
    let prelude = format!(
        "S='{}'\nLOG='{}'\n{BEHAVIOUR}",
        state.display(),
        log.display()
    );
    let (_engine, log) = mock_engine(root, &prelude);
    let mut paths = RikuPaths::for_tests(root);
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        &paths.container_engine_file,
        root.join("engine").display().to_string(),
    )
    .unwrap();
    fs::write(&paths.router_file, "caddy").unwrap();
    fs::create_dir_all(&paths.caddy_root).unwrap();
    let caddy = root.join("mock_caddy");
    executable(&caddy, "#!/bin/sh\nexit 0\n");
    paths.reload_programs.caddy = caddy;
    let app_path = paths.app_root.join(app);
    fs::create_dir_all(&app_path).unwrap();
    fs::write(app_path.join("compose.yaml"), compose).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    fs::write(paths.env_root.join(app).join("ENV"), env).unwrap();
    MockServer { paths, state, log }
}

pub fn myapp() -> AppName {
    AppName::try_from("myapp").unwrap()
}

/// Deploy `myapp` on `server` with its saved ENV.
pub fn deploy(server: &MockServer) -> Result<()> {
    let env = saved_env("myapp", &server.paths).unwrap();
    deploy_containers(&myapp(), &server.paths, &env)
}
