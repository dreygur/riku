//! A mock engine that remembers the containers it ran, for tests of whole
//! container deploys, plus a temp riku root routed through a mock Caddy.
//!
//! State lives in `<root>/state`: per container its name, labels and state
//! JSON. Files a test creates there change how new containers behave:
//! `exits-<service>` makes them exit at once with code 1, `slow-<service>`
//! makes their health report `starting` for the first two inspects. The
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

const BEHAVIOUR: &str = r#"
last=''; for a in "$@"; do last=$a; done
case "$1" in
ps)
  case "$*" in *name=*) exit 0;; esac
  for f in "$S"/*.state; do [ -e "$f" ] || continue; basename "$f" .state; done;;
inspect)
  shift 3
  printf '['; sep=''
  for id in "$@"; do
    [ -e "$S/$id.state" ] || { echo "No such container: $id" >&2; exit 1; }
    state=$(cat "$S/$id.state")
    polls=$(cat "$S/$id.polls" 2>/dev/null || echo 0)
    if [ "$polls" -gt 0 ]; then
      echo $((polls - 1)) > "$S/$id.polls"
      state='{"Status":"running","Health":{"Status":"starting"}}'
    elif [ "$state" = '{"Status":"running"}' ] && [ ! -e "$S/$id.ready" ]; then
      touch "$S/$id.ready"; echo "ready $(cat "$S/$id.name")" >> "$LOG"
    fi
    printf '%s{"Id":"%s","Name":"/%s","Image":"sha256:x","Config":{"Image":"img","Labels":{%s}},"State":%s}' \
      "$sep" "$id" "$(cat "$S/$id.name")" "$(cat "$S/$id.labels")" "$state"
    sep=','
  done
  echo ']';;
run)
  n=$(cat "$S/counter" 2>/dev/null || echo 0); n=$((n + 1)); echo "$n" > "$S/counter"
  id=$(printf '%064d' "$n")
  prev=''; labels=''; svc=''; name=''
  for a in "$@"; do
    case "$prev" in
      --name) name=$a;;
      --label) k=${a%%=*}; v=${a#*=}; labels="$labels${labels:+,}\"$k\":\"$v\""
        [ "$k" = riku.service ] && svc=$v;;
    esac
    prev=$a
  done
  echo "$name" > "$S/$id.name"; printf '%s' "$labels" > "$S/$id.labels"
  if [ -e "$S/exits-$svc" ]; then
    echo '{"Status":"exited","ExitCode":1}' > "$S/$id.state"
  else
    echo '{"Status":"running"}' > "$S/$id.state"
  fi
  [ -e "$S/slow-$svc" ] && echo 2 > "$S/$id.polls"
  echo "$id";;
stop) echo '{"Status":"exited","ExitCode":0}' > "$S/$last.state";;
rm) rm -f "$S/$last".*;;
network)
  case "$2" in
  ls) for f in "$S"/net-*; do [ -e "$f" ] || continue; basename "$f" | cut -c5-; done;;
  create) touch "$S/net-$3";;
  rm) rm -f "$S/net-$3";;
  esac;;
esac
"#;

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
