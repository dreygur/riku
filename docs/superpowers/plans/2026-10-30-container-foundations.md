# Container Services Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tested library surfaces for phase 3 to wire in: an engine agnostic container driver, a compose file reader, a routing plan rendered by nginx, Caddy and router plugins, and riku's own file sync.

**Architecture:** Task 0 adds the shared types: a new `riku-container` crate holding container value types (names, ports, mounts, stop and health settings), a `Backend` type in `riku-util`, and the ETXTBSY spawn helpers moved from `riku-plugins` to `riku-util`. Then four tracks run in parallel worktrees: A fills `riku-container/src/engine/`, B fills `riku-container/src/compose/`, C changes `riku-util`, `riku-caddy`, `riku-nginx`, `riku-bundle` and `riku-deploy/src/router*`, D adds a new `riku-sync` crate. No track touches another track's files.

**Tech Stack:** Rust workspace, std::process for the engine CLI, serde/serde_json, a serde YAML crate (B2), `tar`, `zstd` and `sha2` (D), Tera templates (C2).

**Spec:** `docs/superpowers/specs/2026-10-30-container-services-design.md`

## Global Constraints

- Project rules in `.claude/CLAUDE.md` bind every task: layering, files about 200 lines split by concern, tests in a sibling `_tests.rs` via `#[path]`, no `?` in new or touched functions, doc comments on public items, `//!` module docs with the security model at every trust boundary, no em dashes, no AI attribution in commits (`git commit -F <file> --no-verify`).
- Rule 21 in `.claude/CLAUDE.md`: Read/Edit/Write for files, never python, perl or any script interpreter; shell grep/sed only for search and mechanical renames.
- Confidentiality: never write the owner's real server, organisation or project names anywhere (code, tests, docs, commits, PRs). Only the neutral names listed below.
- Phase 2 changes no user facing behaviour except where a task says so (C3 router plugin protocol, C2 nginx config text that routes identically). Every existing test keeps passing; only `use` paths and the signatures a task names may change.
- Tests are hermetic (rule 20): no real container engine, no PATH or env mutation in the test process. The engine is a fake executable (`/bin/sh` script) written into the test's own temp dir and passed by path; detection takes the search path as a parameter.
- Engine contract (spec, verbatim): "Riku drives engines through the Docker compatible CLI: `pull`, `build`, `run -d`, `stop -t`, `kill`, `rm`, `ps --format json`, `inspect`, `stats --no-stream --format json`, `logs -f`, `version`, `network`, `volume`." "One driver, no per engine branches." "Where that happens the driver probes capability, never the engine name."
- Owner decision 1 (verbatim): "Published ports bind to 127.0.0.1 by default, so a database's `5432:5432` is no longer reachable from outside the server. A service is exposed on all interfaces only when its compose file opts in with the label `riku.public: "true"`. The routed HTTP port is the published port of the service that has the app's domains; a `riku.http: "<port>"` label picks it when that is ambiguous, and an ambiguous service without the label is a deploy error."
- Owner decision 2 (verbatim): "Services that are not routed (workers, schedulers) are replaced stop first during a rolling deploy: the old copy stops, then the new one starts, with no overlap, so scheduled jobs never fire twice. Only routed web copies overlap." Phase 2 only classifies (B4); phase 3 acts on it.
- "Storage services cannot scale. A service that mounts any writable volume or bind mount is refused with a message saying why. Read only mounts do not count." "A scaled service that publishes a non HTTP port is refused: routers only balance HTTP."
- "Any other key is a deploy error naming the key and service." Top level `version`, `volumes`, `networks` and `x-*` keys, `${VAR}` and `${VAR:-default}` interpolation and the project `.env` file are supported.
- "Containers are named `<app>-<service>-<n>`, carry labels identifying app, service and copy, and join one network per app." Containers run with the engine's restart policy off (`--restart no`); the supervisor owns restarts.
- Default stop grace period 30 s; default stop signal SIGTERM. Removal never deletes volumes (no `-v`/`--volumes` on `rm`).
- Router plugin protocol: "changes from one upstream port to a list of backends and gets a protocol version"; "a plugin router must say whether it [its reload] is" graceful.
- Sync: "rsync is not assumed present"; manifest of path, size, content hash; server replies with needed and deleted files; one zstd compressed tar stream; "swaps it in only after the whole transfer is verified".
- Neutral names only in code, tests and docs: server `edge`, host `edge.example`, apps `myapp`, `shop`, services `web`, `worker`, `proxy`, `db`.
- Gate before every commit: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -q`. Record the baseline pass count at the branch point in Task 0's commit body; every later gate reports baseline plus the task's new tests, 0 failed.
- Commit messages: imperative, under 50 characters.
- Owner decisions 2026-10-30 (answers to questions the spec left open):
  - Routed service: the service labelled `riku.http` wins; with no label, routing works only when exactly one service publishes exactly one port; anything else is a deploy error naming the services.
  - The top level `name` key is accepted, so migrating an existing compose app keeps its volume names.
  - Interpolation is compose compatible: `$VAR`, `${VAR}`, `${VAR:-default}`, `$$` and the project `.env` file; other `${...}` forms are refused; an unset variable gives a warning, as compose does.

## Review Focus

1. A compose file that uses YAML anchors in an `x-` block and `<<:` merge keys inside services: expect the merged keys applied, and the unknown key check to run on the merged result. Test in B2.
2. An environment value containing a newline, or a key containing `=`: expect a refusal naming the key, never a value split into extra engine flags; and no secret value in the engine argv. Tests in A3.
3. A hostile or broken sync sender (a path with `..`, a file under a directory that the manifest declares as a symlink, more bytes than the manifest size, a path it was not asked for, a stream cut halfway): expect a refusal with the live directory byte for byte unchanged and no staging directory left. Tests in D2 and D3.
4. `riku.public` set to anything other than `"true"` or `"false"`, or a compose port with an explicit host IP (`"0.0.0.0:80:80"`): expect a deploy error naming service and value, never a silent public bind. Tests in B3 and B4.
5. An existing compose app whose `COMPOSE_FILE` sits in a subdirectory: expect the existing volume names to use that directory's name as the project, so migration finds the data. Test in B4.

---

## Order and worktrees

1. Create branch `feat/container-foundations` from the tip that contains phase 1 (`feat/caddy-router` at or after 4b120b0). Task 0 runs there and lands first.
2. After Task 0, create four worktrees from that commit (superpowers:using-git-worktrees): `feat/cf-engine` (track A), `feat/cf-compose` (track B), `feat/cf-routing` (track C), `feat/cf-sync` (track D). Tracks run at the same time; tasks inside a track run in order.
3. Track D consumes nothing from Task 0; it still branches after Task 0 so every track starts from the same base.
4. Merge the tracks into `feat/container-foundations` in any order. Expected conflicts: `Cargo.lock` only (B and D add dependencies). Resolve by taking the target branch's `Cargo.lock` and running `cargo build -q` to regenerate it; then run the gate.
5. ROADMAP: Task 0 writes the phase 2 status block with one line per task, one blank line between track groups, so each track edits only its own lines. Each task flips its own line to `done` in its commit; the coordinator adds commit hashes after merging.

## File Structure

```
crates/riku-container/                 new (Task 0): container value types, engine driver, compose reader
  src/lib.rs                           pub mod types; pub mod engine; pub mod compose;
  src/types/{mod.rs,names.rs,ports.rs,mounts.rs,lifecycle.rs}   Task 0
  src/engine/mod.rs                    Task 0 writes the //! doc only; A fills it
  src/engine/{detect.rs,identity.rs,probe.rs,run_args.rs,env_file.rs,control.rs,inspect.rs,stats.rs,fake.rs}   A
  src/compose/mod.rs                   Task 0 writes the //! doc only; B fills it
  src/compose/{locate.rs,raw.rs,interpolate.rs,keys.rs,service.rs,ports.rs,volumes.rs,health.rs,duration.rs,environment.rs,order.rs,rules.rs,project.rs}   B
crates/riku-util/src/backend.rs        Task 0: Backend
crates/riku-util/src/spawn.rs          Task 0: ETXTBSY helpers moved from riku-plugins/src/executor.rs
crates/riku-util/src/route_plan.rs     C1: RoutePlan, HttpsPolicy (moved from riku-caddy)
crates/riku-config/src/container_engine.rs   A1: CONTAINER_ENGINE setting file
crates/riku-caddy/src/site.rs          C1: render from RoutePlan
crates/riku-nginx/src/backends.rs      C2: default backends for an app
templates/nginx_upstream.conf.tera     C2: shared upstream block
crates/riku-bundle/src/router_section.rs   C3: [router] manifest section
crates/riku-deploy/src/router.rs, router/caddy.rs, router/plugin.rs   C1, C3
crates/riku-sync/                      new (D1): manifest, transfer plan, bundle, apply, session
crates/riku-plugin-container/src/compose_file.rs   B1: removed, plugin uses riku_container::compose::locate
```

---

### Task 0: Shared types (lands before every track)

**Files:**
- Create: `crates/riku-container/{Cargo.toml,src/lib.rs}`, `crates/riku-container/src/types/{mod.rs,names.rs,names_tests.rs,ports.rs,mounts.rs,mounts_tests.rs,lifecycle.rs,lifecycle_tests.rs}`, `crates/riku-container/src/engine/mod.rs`, `crates/riku-container/src/compose/mod.rs`
- Create: `crates/riku-util/src/backend.rs`, `crates/riku-util/src/backend_tests.rs`, `crates/riku-util/src/spawn.rs`
- Modify: root `Cargo.toml` (member `crates/riku-container`), `crates/riku-util/src/lib.rs`, `crates/riku-plugins/src/executor.rs` (re-export the moved helpers), `docs/superpowers/ROADMAP.md`, `.claude/CLAUDE.md` (crate list line for riku-container)

**Interfaces:**
- Consumes: `riku_util::validate_app_name(&str) -> anyhow::Result<String>`.
- Produces (all `pub`, in `riku_container::types`):
  - `ServiceName(String)`: `impl TryFrom<&str>` (first char ASCII alphanumeric, rest ASCII alphanumeric or `_ . -`, 1 to 63 chars), `as_str()`, `Display`.
  - `CopyIndex(u32)`: `fn new(n: u32) -> Option<CopyIndex>` (None for 0), `get() -> u32`, `Display`.
  - `ContainerName(String)`: `fn new(app: &str, service: &ServiceName, copy: CopyIndex) -> anyhow::Result<ContainerName>` (fails when `validate_app_name(app)` fails or changes the value), `as_str()`.
  - `NetworkName(String)`: `fn for_app(app: &str) -> NetworkName` (`riku-<app>`), `as_str()`.
  - `mod labels { pub const APP: &str = "riku.app"; SERVICE = "riku.service"; COPY = "riku.copy"; PUBLIC = "riku.public"; HTTP = "riku.http"; HEALTH = "riku.health"; }`
  - `enum HostIp { Loopback, All }`, `enum Protocol { Tcp, Udp }`, `struct PortBinding { pub host_ip: HostIp, pub host_port: Option<u16>, pub container_port: u16, pub protocol: Protocol }` (`host_port: None` means the engine picks).
  - `enum MountSource { Volume(String), Bind(PathBuf), Anonymous }`, `struct Mount { pub source: MountSource, pub target: PathBuf, pub read_only: bool }`, `Mount::is_writable(&self) -> bool`.
  - `StopSignal(String)`: `impl TryFrom<&str>` (uppercased, must match `SIG[A-Z0-9]+`), `Default` = `SIGTERM`, `as_str()`. `pub const DEFAULT_STOP_GRACE: Duration = Duration::from_secs(30);`
  - `enum RestartPolicy { No, Always, OnFailure { max_retries: Option<u32> }, UnlessStopped }` (`Default` = `No`).
  - `enum HealthTest { Exec(Vec<String>), Shell(String), Disabled }`, `struct HealthCheck { pub test: HealthTest, pub interval: Option<Duration>, pub timeout: Option<Duration>, pub retries: Option<u32>, pub start_period: Option<Duration> }`.
- Produces in `riku-util`: `riku_util::backend::Backend { Tcp(u16), Unix(PathBuf) }` with `Display` (`127.0.0.1:<port>` / `unix:<path>`); `riku_util::spawn::{spawn_retrying_etxtbsy(&mut Command) -> io::Result<Child>, output_retrying_etxtbsy(&mut Command) -> io::Result<Output>}` (moved verbatim; `riku_plugins::executor` re-exports both at the old path).

- [ ] **Step 1: Write the failing tests**

```rust
// types/names_tests.rs
#[test]
fn container_name_joins_app_service_and_copy() {
    let web = ServiceName::try_from("web").unwrap();
    let name = ContainerName::new("myapp", &web, CopyIndex::new(1).unwrap()).unwrap();
    assert_eq!(name.as_str(), "myapp-web-1");
}
#[test]
fn container_name_rejects_unvalidated_app() {
    let web = ServiceName::try_from("web").unwrap();
    assert!(ContainerName::new("../myapp", &web, CopyIndex::new(1).unwrap()).is_err());
}
#[test]
fn service_name_rejects_engine_unsafe_characters() {
    for bad in ["", "-web", "web/../x", "we b", "web;rm"] {
        assert!(ServiceName::try_from(bad).is_err(), "accepted {bad:?}");
    }
    for good in ["web", "db_1", "api.v2"] {
        assert!(ServiceName::try_from(good).is_ok(), "rejected {good:?}");
    }
}
#[test]
fn copy_index_starts_at_one() { assert!(CopyIndex::new(0).is_none()); }
#[test]
fn network_is_one_per_app() { assert_eq!(NetworkName::for_app("myapp").as_str(), "riku-myapp"); }

// types/mounts_tests.rs
#[test]
fn only_read_only_mounts_are_not_storage() {
    // volume rw -> true, bind ro -> false, anonymous -> true
}

// types/lifecycle_tests.rs
#[test]
fn stop_signal_defaults_to_sigterm_and_rejects_junk() {
    assert_eq!(StopSignal::default().as_str(), "SIGTERM");
    assert_eq!(StopSignal::try_from("sigint").unwrap().as_str(), "SIGINT");
    assert!(StopSignal::try_from("TERM; rm").is_err());
    assert_eq!(DEFAULT_STOP_GRACE, Duration::from_secs(30));
}

// riku-util backend_tests.rs
#[test]
fn backend_renders_router_address() {
    assert_eq!(Backend::Tcp(5000).to_string(), "127.0.0.1:5000");
    assert_eq!(Backend::Unix("/srv/riku/nginx/myapp.sock".into()).to_string(), "unix:/srv/riku/nginx/myapp.sock");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p riku-container -p riku-util`
Expected: compile errors (crate and types missing).

- [ ] **Step 3: Implement the types listed under Produces.** `riku-container` dependencies: anyhow, serde, serde_json, tracing, riku-util, riku-config; dev: tempfile. `engine/mod.rs` and `compose/mod.rs` carry only their `//!` docs (A and B own the bodies): engine doc states that every value reaches the engine as a separate argv element, never through a shell; compose doc states that the compose file is app code from a git push and every value is validated before it reaches the engine. Move `retry_etxtbsy` and both public wrappers to `riku-util/src/spawn.rs` with their doc comments; `riku-plugins` re-exports.

- [ ] **Step 4: ROADMAP and CLAUDE.md.** Add the phase 2 block: `Plan plans/2026-10-30-container-foundations.md`, then groups `Task 0`, `A1..A4`, `B1..B4`, `C1..C3`, `D1..D3`, each line `- <id> <name>: pending`, one blank line between groups; set Task 0 to `done`. Add the riku-container line to the crate list in `.claude/CLAUDE.md`.

- [ ] **Step 5: Gate.** Expected: baseline plus the new tests, 0 failed.

- [ ] **Step 6: Commit** `Add shared container and backend types`

---

## Track A: engine driver (`riku-container/src/engine/`)

### Task A1: Engine detection and the engine setting

**Files:**
- Create: `crates/riku-config/src/container_engine.rs`, `container_engine_tests.rs`; `crates/riku-container/src/engine/detect.rs`, `detect_tests.rs`, `fake.rs` (`#[cfg(test)]` helper)
- Modify: `crates/riku-config/src/lib.rs` (field `container_engine_file`, set in `from_dirs`), `crates/riku-container/src/engine/mod.rs`

**Interfaces:**
- Consumes: `RikuPaths`.
- Produces:
  - `riku_config::container_engine::{ENGINE_FILE: &str = "CONTAINER_ENGINE", load(paths: &RikuPaths) -> anyhow::Result<Option<String>>}` (missing file: `None`; empty after trim: `None`; other read errors: `Err`).
  - `riku_container::engine::KNOWN_ENGINES: [&str; 4] = ["docker", "podman", "nerdctl", "finch"]`.
  - `pub struct EngineBinary { pub program: PathBuf, pub binary_name: String }`.
  - `pub fn detect(setting: Option<&str>, search_path: &OsStr) -> anyhow::Result<EngineBinary>`.
  - `#[cfg(test)] fake::fake_engine(dir: &Path, name: &str, script: &str) -> PathBuf` (writes `#!/bin/sh` plus `script`, mode 0755, returns the path).

Detection rule: a setting that is an absolute path must be an executable file; a bare name is looked up in `search_path`; anything else (relative path with `/`, whitespace) is refused. No setting: first of `KNOWN_ENGINES` found executable in `search_path` order wins.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn picks_first_known_engine_on_path() {
    // dir1 holds podman, dir2 holds docker; search_path = dir1:dir2
    // expect binary_name "docker" (list order beats PATH order)
}
#[test]
fn setting_names_the_engine() {
    // setting Some("nerdctl"), nerdctl and docker both present -> "nerdctl"
}
#[test]
fn setting_that_is_not_executable_is_refused() {
    // setting Some("/tmp/x/notexec") with mode 0644 -> Err containing "CONTAINER_ENGINE" and the value
}
#[test]
fn no_engine_lists_what_was_searched() {
    let err = detect(None, OsStr::new(empty_dir)).unwrap_err().to_string();
    assert!(err.contains("docker, podman, nerdctl, finch"), "{err}");
}
// container_engine_tests.rs
#[test]
fn missing_engine_file_means_detect() { assert_eq!(load(&paths).unwrap(), None); }
```

- [ ] **Step 2:** `cargo test -q -p riku-container engine::detect -p riku-config container_engine` fails (missing items).
- [ ] **Step 3:** Implement. No env reads inside `detect`; the caller passes `PATH`.
- [ ] **Step 4:** Same command passes; gate.
- [ ] **Step 5:** Flip A1 in ROADMAP. Commit `Detect the container engine`

### Task A2: Engine identity and command probe

**Files:**
- Create: `crates/riku-container/src/engine/identity.rs`, `identity_tests.rs`, `probe.rs`, `probe_tests.rs`
- Modify: `crates/riku-container/src/engine/mod.rs`

**Interfaces:**
- Consumes: `EngineBinary`, `fake_engine`, `riku_util::spawn::output_retrying_etxtbsy`.
- Produces:
  - `pub struct EngineIdentity { pub name: String, pub version: String }`.
  - `pub fn parse_identity(binary_name: &str, version_json: &str) -> anyhow::Result<EngineIdentity>`: name is `Server.Platform.Name` when present and not empty, else `binary_name`; version is `Server.Version`, else `Client.Version`, else error.
  - `pub const REQUIRED_COMMANDS: [&str; 13] = ["pull", "build", "run", "stop", "kill", "rm", "ps", "inspect", "stats", "logs", "version", "network", "volume"];`
  - `pub struct Engine { program: PathBuf, identity: EngineIdentity }` with `pub fn open(binary: EngineBinary) -> anyhow::Result<Engine>` (probe, then `version --format json`), `pub fn identity(&self) -> &EngineIdentity`, `pub(crate) fn output(&self, args: &[OsString]) -> anyhow::Result<String>` (non zero exit: error with the command name and trimmed stderr).

Probe: for each required command run `<program> <command> --help`; the first non zero exit or spawn failure is refused with `container engine <program> does not answer '<command>'; riku needs: pull, build, run, ...`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn docker_identity_uses_platform_name() {
    let json = r#"{"Client":{"Version":"27.3.1"},"Server":{"Platform":{"Name":"Docker Engine - Community"},"Version":"27.3.1"}}"#;
    let id = parse_identity("docker", json).unwrap();
    assert_eq!((id.name.as_str(), id.version.as_str()), ("Docker Engine - Community", "27.3.1"));
}
#[test]
fn engine_without_product_name_shows_binary_name() {
    let json = r#"{"Client":{"Version":"5.2.2"},"Server":{"Version":"5.2.2"}}"#;
    assert_eq!(parse_identity("podman", json).unwrap().name, "podman");
}
#[test]
fn open_refuses_engine_missing_a_command() {
    // fake engine: `case "$1" in stats) exit 1;; esac; exit 0`
    let err = Engine::open(bin).unwrap_err().to_string();
    assert!(err.contains("'stats'"), "{err}");
}
#[test]
fn open_reads_identity_after_probe() {
    // fake engine prints the podman JSON for `version`, exits 0 for every `--help`
    assert_eq!(Engine::open(bin).unwrap().identity().version, "5.2.2");
}
```

Before merging, replace the two JSON fixtures with output captured from a real `docker version --format json` and `podman version --format json` (a podman container works), keeping the assertions; say in the commit body which fixture is captured and which is from docs.

- [ ] **Step 2:** `cargo test -q -p riku-container engine::` fails.
- [ ] **Step 3:** Implement. Spawn through `output_retrying_etxtbsy` (the fake engine is written moments before it runs).
- [ ] **Step 4:** Tests pass; gate. Run the four tests 20 times with `--test-threads 1` and default threads to check for ETXTBSY flakes.
- [ ] **Step 5:** Flip A2. Commit `Probe engine commands and identity`

### Task A3: Run and control containers

**Files:**
- Create: `crates/riku-container/src/engine/run_args.rs`, `run_args_tests.rs`, `env_file.rs`, `env_file_tests.rs`, `control.rs`, `control_tests.rs`
- Modify: `crates/riku-container/src/engine/mod.rs`

**Interfaces:**
- Consumes: Task 0 types, `Engine::output`.
- Produces:
  - `pub struct RunSpec { pub name: ContainerName, pub image: String, pub network: NetworkName, pub alias: ServiceName, pub labels: BTreeMap<String, String>, pub env: BTreeMap<String, String>, pub ports: Vec<PortBinding>, pub mounts: Vec<Mount>, pub entrypoint: Option<Vec<String>>, pub command: Option<Vec<String>>, pub working_dir: Option<String>, pub user: Option<String>, pub stop_signal: StopSignal, pub stop_grace: Duration, pub healthcheck: Option<HealthCheck> }`
  - `pub struct ContainerId(String)` (from engine stdout, trimmed, must be hex), `as_str()`.
  - `pub(crate) fn run_args(spec: &RunSpec, env_file: &Path) -> Vec<OsString>`
  - `pub(crate) fn write_env_file(dir: &Path, env: &BTreeMap<String, String>) -> anyhow::Result<tempfile::TempPath>` (mode 0600; refuses a key that is empty or holds `=`, whitespace or NUL, and a value holding `\n`, `\r` or NUL, naming the key)
  - `impl Engine`: `run(&self, spec: &RunSpec, scratch: &Path) -> Result<ContainerId>`, `stop(&self, id: &ContainerId, grace: Duration) -> Result<()>` (`stop -t <secs>`), `kill(&self, id, signal: &StopSignal) -> Result<()>` (`kill --signal <sig>`), `remove(&self, id) -> Result<()>` (`rm <id>`), `pull(&self, image: &str) -> Result<()>`, `build(&self, spec: &BuildSpec) -> Result<()>`, `ensure_network(&self, name: &NetworkName) -> Result<()>` (`network inspect`, else `network create`), `volume_exists(&self, name: &str) -> Result<bool>` (`volume inspect` exit code), `logs_command(&self, id: &ContainerId, follow: bool, tail: Option<u32>) -> Command`.
  - `pub struct BuildSpec { pub context: PathBuf, pub dockerfile: Option<PathBuf>, pub tag: String, pub args: BTreeMap<String, String>, pub platform: Option<String> }`

`run_args` order: `run -d --name <name> --restart no --network <net> --network-alias <alias> --label k=v... (app, service, copy labels come from the caller) --env-file <file> -p ... -v ... [--workdir] [--user] --stop-signal <sig> --stop-timeout <secs> [health flags] [--entrypoint <first>] <image> [entrypoint[1..]] [command...]`. Port flag: `127.0.0.1:<host>:<container>/<proto>` for `Loopback`, `<host>:<container>/<proto>` for `All`, an empty host port when `host_port` is `None` (`127.0.0.1::80/tcp`). Mount flag: `<volume>:<target>[:ro]`, `<abs bind>:<target>[:ro]`, `<target>` for anonymous. Health: `Shell(s)` gives `--health-cmd s`; `Exec(v)` gives `--health-cmd` with `v` joined by `shlex::try_join` (the CLI has no exec form; an image without `/bin/sh` reports unhealthy, documented on `RunSpec`); `Disabled` gives `--no-healthcheck`; durations as `<n>s`. No capability branch is added: none of these flags differs between the engines in the contract. If a real engine run during review shows one does, add a probe, never a name check.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn published_ports_bind_loopback_unless_public() {
    let args = joined(run_args(&spec_with_ports(HostIp::Loopback), env_file));
    assert!(args.contains("-p 127.0.0.1:5432:5432/tcp"), "{args}");
    let args = joined(run_args(&spec_with_ports(HostIp::All), env_file));
    assert!(args.contains("-p 5432:5432/tcp"), "{args}");
}
#[test]
fn engine_restart_policy_is_off() { assert!(joined(run_args(&spec(), f)).contains("--restart no")); }
#[test]
fn entrypoint_list_splits_into_flag_and_leading_args() {
    // entrypoint ["python", "-u"], command ["app.py"] -> ends with "--entrypoint python <image> -u app.py"
}
#[test]
fn secrets_never_appear_in_argv() {
    // env {"DATABASE_URL": "postgres://u:secret@db/x"}: no argv element contains "secret"; "--env-file" present
}
#[test]
fn env_value_with_newline_is_refused() {
    let err = write_env_file(dir, &env([("KEY", "a\nb")])).unwrap_err().to_string();
    assert!(err.contains("KEY"), "{err}");
}
#[test]
fn remove_never_deletes_volumes() {
    // fake engine logs "$@" to a file; after engine.remove(&id) the log line is exactly "rm <id>"
}
#[test]
fn stop_passes_grace_seconds() { /* log line "stop -t 30 <id>" for DEFAULT_STOP_GRACE */ }
#[test]
fn run_returns_the_engine_container_id() { /* fake prints 64 hex chars for `run`; ContainerId matches */ }
```

- [ ] **Step 2:** `cargo test -q -p riku-container engine::` fails.
- [ ] **Step 3:** Implement; the env file lives in `scratch` and is deleted when `run` returns (the engine reads it during `run`).
- [ ] **Step 4:** Tests pass; gate.
- [ ] **Step 5:** Flip A3. Commit `Run and control containers via engine CLI`

### Task A4: Container state and stats

**Files:**
- Create: `crates/riku-container/src/engine/inspect.rs`, `inspect_tests.rs`, `stats.rs`, `stats_tests.rs`
- Modify: `crates/riku-container/src/engine/mod.rs`

**Interfaces:**
- Consumes: `Engine::output`, `ContainerId`, `labels`, `ServiceName`, `CopyIndex`.
- Produces:
  - `pub enum ContainerState { Running, Unhealthy, Restarting, Exited { code: i32 }, Created, Other(String) }`
  - `pub struct ContainerInfo { pub id: ContainerId, pub name: String, pub service: Option<ServiceName>, pub copy: Option<CopyIndex>, pub image: String, pub state: ContainerState }`
  - `pub struct ContainerStats { pub id: String, pub cpu_percent: f64, pub memory_bytes: u64 }`
  - `pub(crate) fn parse_inspect(json: &str) -> anyhow::Result<Vec<ContainerInfo>>`, `pub(crate) fn parse_stats(output: &str) -> anyhow::Result<Vec<ContainerStats>>`
  - `impl Engine`: `list(&self, app: &str) -> Result<Vec<ContainerId>>` (`ps -a -q --no-trunc --filter label=riku.app=<app>`), `inspect(&self, ids: &[ContainerId]) -> Result<Vec<ContainerInfo>>`, `stats(&self, ids: &[ContainerId]) -> Result<Vec<ContainerStats>>` (`stats --no-stream --format json <ids>`).

Decisions: listing uses `ps -q` plus `inspect` because `ps --format json` differs in shape between engines (one object per line with a `Labels` string against a JSON array with a `Labels` map) while `inspect` follows the Docker API shape in both. `name` strips a leading `/`. `Unhealthy` when `State.Status` is `running` and `State.Health.Status` is `unhealthy`. `parse_stats` accepts one JSON object per line or a JSON array, keys `ID`/`id`, `CPUPerc`/`cpu_percent`, `MemUsage`/`mem_usage`; memory is the part before ` / `, units `B`, `kB`, `KiB`, `MB`, `MiB`, `GB`, `GiB`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn inspect_reads_state_labels_and_image() {
    // fixture: [{"Id":"abc..","Name":"/myapp-web-2","Config":{"Image":"shop/web:1.4","Labels":{"riku.service":"web","riku.copy":"2"}},
    //            "State":{"Status":"running","ExitCode":0,"Health":{"Status":"unhealthy"}}}]
    // expect name "myapp-web-2", service "web", copy 2, image "shop/web:1.4", state Unhealthy
}
#[test]
fn exited_container_keeps_its_code() { /* Status "exited", ExitCode 137 -> Exited { code: 137 } */ }
#[test]
fn stats_parse_docker_lines() {
    let out = "{\"ID\":\"abc\",\"CPUPerc\":\"0.50%\",\"MemUsage\":\"21.5MiB / 7.6GiB\"}\n";
    let s = &parse_stats(out).unwrap()[0];
    assert_eq!((s.cpu_percent, s.memory_bytes), (0.5, 22_544_384));
}
#[test]
fn stats_parse_json_array() { /* same values as an array with lowercase keys and "21.5MB / 8GB" -> 21_500_000 */ }
#[test]
fn list_filters_by_app_label() { /* fake engine logs argv; expect "ps -a -q --no-trunc --filter label=riku.app=myapp" */ }
```

Before merging, capture real `inspect` and `stats` output from Docker and from Podman (in a container) and add each as a fixture case with the same assertions; name the source of each fixture in the commit body.

- [ ] **Step 2:** fails. **Step 3:** implement. **Step 4:** pass; gate.
- [ ] **Step 5:** Flip A4. Commit `Read container state and stats`

---

## Track B: compose reader (`riku-container/src/compose/`)

### Task B1: Move compose file lookup into the library

**Files:**
- Move: `crates/riku-plugin-container/src/compose_file.rs` and `compose_file_tests.rs` to `crates/riku-container/src/compose/locate.rs` and `locate_tests.rs` (`git mv`)
- Modify: `crates/riku-container/src/compose/mod.rs`, `crates/riku-plugin-container/{Cargo.toml,src/main.rs,src/compose_cli.rs}`

**Interfaces:**
- Produces: `riku_container::compose::{ComposeFile, COMPOSE_FILE_NAMES, default_compose_file(app_path: &Path) -> Option<ComposeFile>, find_compose_file(app_path: &Path, requested: Option<&str>) -> anyhow::Result<Option<ComposeFile>>}`, `ComposeFile::arg(&self) -> String`, `ComposeFile::relative(&self) -> &Path`. The `COMPOSE_FILE` env read stays in the plugin (`requested_compose_file`) so the library never reads process env.

- [ ] **Step 1:** Move the file and its tests; tests call `find_compose_file(dir, requested)` directly. Run `cargo test -q -p riku-container compose::locate`: the six moved tests pass under the new path.
- [ ] **Step 2:** The plugin depends on riku-container and keeps `compose_file(app_path)` as a two line wrapper over `find_compose_file(app_path, requested_compose_file().as_deref())`. `cargo test -q -p riku-plugin-container` passes unchanged except `use` lines.
- [ ] **Step 3:** Gate. Flip B1. Commit `Move compose file lookup into riku-container`

### Task B2: Read compose YAML with interpolation and key checks

**Files:**
- Create: `crates/riku-container/src/compose/{raw.rs,raw_tests.rs,interpolate.rs,interpolate_tests.rs,keys.rs,keys_tests.rs}`
- Modify: `crates/riku-container/Cargo.toml` (YAML crate), `compose/mod.rs`

**Interfaces:**
- Consumes: `ComposeFile`, `riku_util::env::parse_settings` (for `.env`), `riku_util::ensure_path_within`.
- Produces:
  - `pub struct RawProject { pub services: BTreeMap<String, Value>, pub volumes: BTreeMap<String, Value>, pub name: Option<String>, pub warnings: Vec<String> }` (`Value` is the YAML crate's value type, re-exported as `compose::Value`).
  - `pub fn read_raw(app_path: &Path, compose: &ComposeFile, app_env: &BTreeMap<String, String>) -> anyhow::Result<RawProject>`
  - `pub(crate) fn interpolate(text: &str, vars: &BTreeMap<String, String>, warnings: &mut Vec<String>) -> anyhow::Result<String>`
  - `pub(crate) const SERVICE_KEYS: [&str; 17] = ["build", "command", "depends_on", "entrypoint", "env_file", "environment", "healthcheck", "image", "labels", "networks", "ports", "restart", "stop_grace_period", "stop_signal", "user", "volumes", "working_dir"];` (the spec's version 1 list) and `pub(crate) fn check_keys(project: &Mapping) -> anyhow::Result<()>` (`Mapping` is the YAML crate's map type).

YAML crate: use `serde_yaml_ng` if `cargo info serde_yaml_ng` shows a release in the last year and `Value::apply_merge`; otherwise choose another serde YAML crate with merge key support and say why in the commit body. Record the release size delta of `riku` (`cargo build --release -p riku`, before and after) in the commit body.

Rules:
- Variables: the project `.env` (next to the compose file) overlaid by `app_env`. `${VAR}`, `$VAR`, `${VAR:-default}` (default when unset or empty), `$$` gives `$`. `${VAR-x}`, `${VAR:?x}`, `${VAR?x}`, `${VAR:+x}`, `${VAR+x}` are errors naming the expression. An unset variable becomes empty and adds the warning `variable VAR is not set; using an empty string` (compose behaviour). Interpolation runs on scalar values after YAML parsing, never on keys.
- Merge keys are applied before the key check.
- Top level keys allowed: `services`, `version`, `volumes`, `networks`, `name`, and any `x-*`. `name` is not in the spec list; it is read because it sets the existing volume names (B4) and refusing it would block those apps from migrating (owner decision, see Global Constraints).
- Top level `networks` may only declare `default`, and `default` may carry no settings.
- Service keys outside `SERVICE_KEYS` fail with `service '<service>': unsupported key '<key>'`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn default_and_dotenv_values_are_interpolated() {
    // .env "TAG=1.4", app_env {"PORT":"8000"}; image "shop/web:${TAG}", command "serve --port ${PORT:-5000} ${HOST:-0.0.0.0}"
    // expect "shop/web:1.4" and "serve --port 8000 0.0.0.0"
}
#[test]
fn app_env_overrides_dotenv() { /* .env TAG=1.4, app_env TAG=1.5 -> "1.5" */ }
#[test]
fn double_dollar_is_a_literal_dollar() { assert_eq!(interpolate("a$$b", &vars, &mut w).unwrap(), "a$b"); }
#[test]
fn unsupported_interpolation_is_an_error() {
    let err = interpolate("${DB:?missing}", &vars, &mut w).unwrap_err().to_string();
    assert!(err.contains("${DB:?missing}"), "{err}");
}
#[test]
fn unset_variable_warns() { /* "${NOPE}" -> "" and warnings == ["variable NOPE is not set; using an empty string"] */ }
#[test]
fn unknown_service_key_names_key_and_service() {
    // services.worker.deploy: {replicas: 2}
    let err = read_raw(..).unwrap_err().to_string();
    assert_eq!(err, "service 'worker': unsupported key 'deploy'");
}
#[test]
fn merge_keys_from_x_blocks_are_applied() {
    // x-base: &base {restart: always}; services.web: {<<: *base, image: nginx}
    // expect services["web"]["restart"] == "always"
}
#[test]
fn custom_network_is_refused() { /* top level networks: {backend: {}} -> Err containing "backend" */ }
```

- [ ] **Step 2:** fails. **Step 3:** implement. **Step 4:** pass; gate.
- [ ] **Step 5:** Flip B2. Commit `Read compose YAML with interpolation`

### Task B3: Parse service fields

**Files:**
- Create: `crates/riku-container/src/compose/{service.rs,service_tests.rs,ports.rs,ports_tests.rs,volumes.rs,volumes_tests.rs,health.rs,health_tests.rs,duration.rs,duration_tests.rs,environment.rs,environment_tests.rs}`
- Modify: `compose/mod.rs`

**Interfaces:**
- Consumes: `RawProject`, Task 0 types, `shlex`, `parse_settings`, `ensure_path_within`.
- Produces:
  - `pub struct ServiceDef { pub name: ServiceName, pub image: Option<String>, pub build: Option<BuildDef>, pub command: Option<Vec<String>>, pub entrypoint: Option<Vec<String>>, pub environment: BTreeMap<String, String>, pub ports: Vec<PortBinding>, pub mounts: Vec<Mount>, pub depends_on: Vec<Dependency>, pub healthcheck: Option<HealthCheck>, pub restart: RestartPolicy, pub stop_grace: Duration, pub stop_signal: StopSignal, pub labels: BTreeMap<String, String>, pub working_dir: Option<String>, pub user: Option<String> }`
  - `pub struct BuildDef { pub context: PathBuf, pub dockerfile: Option<PathBuf>, pub args: BTreeMap<String, String> }`
  - `pub struct Dependency { pub service: ServiceName, pub condition: DependsCondition }`, `pub enum DependsCondition { Started, Healthy }`
  - `pub fn parse_service(app_path: &Path, name: &str, value: &Value, declared_volumes: &BTreeSet<String>) -> anyhow::Result<ServiceDef>`
  - `pub(crate) fn parse_duration(raw: &str) -> anyhow::Result<Duration>` (units `us`, `ms`, `s`, `m`, `h`, combined like `1m30s`)

Rules (every error names the service and the offending value):
- Image or build required. `build` as a string (context) or a map of `context`, `dockerfile`, `args`; other build keys refused. Context and dockerfile must resolve inside the app directory.
- `command`/`entrypoint`: string split with `shlex::split` (unbalanced quotes refused) or list.
- `environment`: map (scalar values as strings, null skipped) or list `K=V`; a bare `K` takes the value from the interpolation variables, skipped when unset. `env_file`: string or list, each file inside the app directory, read with `parse_settings`; `environment` wins over `env_file`.
- `ports`: short `"C"`, `"H:C"`, `"H:C/udp"`, or long `{target, published, protocol}`. A host IP in either form, a range, or long syntax `host_ip`/`mode` is refused with `use the riku.public label instead of a host IP`. `host_ip` comes from the `riku.public` label: `"true"` gives `All`, absent or `"false"` gives `Loopback`, any other value is refused.
- `volumes`: short `name:/t[:ro]`, `./rel:/t[:ro]`, `/abs:/t[:ro]`, `/t` (anonymous), or long `{type: volume|bind, source, target, read_only}`; `tmpfs` and other types refused. Relative bind sources resolve against the compose file directory and must stay inside the app directory. A named volume missing from top level `volumes` is refused.
- `depends_on`: list or map with `condition: service_started|service_healthy`; other conditions refused; unknown services refused (checked in B4 where all names are known).
- `healthcheck`: `test` string (`Shell`), list starting `CMD` (`Exec`), `CMD-SHELL` (`Shell`), `NONE` (`Disabled`); `disable: true` gives `Disabled`; `interval`, `timeout`, `start_period` durations; `retries`; other keys refused.
- `restart`: `no`, `always`, `on-failure`, `on-failure:<n>`, `unless-stopped`. `stop_grace_period` defaults to `DEFAULT_STOP_GRACE`. `stop_signal` through `StopSignal::try_from`. `labels` map or list `k=v`. `networks` list or map naming only `default`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn database_port_binds_loopback_by_default() {
    // db: {image: postgres:16, ports: ["5432:5432"]}
    assert_eq!(svc.ports, vec![PortBinding { host_ip: HostIp::Loopback, host_port: Some(5432), container_port: 5432, protocol: Protocol::Tcp }]);
}
#[test]
fn public_label_exposes_on_all_interfaces() { /* labels {"riku.public": "true"} -> HostIp::All */ }
#[test]
fn public_label_with_other_value_is_refused() {
    // labels {"riku.public": "yes"} -> Err containing "service 'proxy'" and "riku.public" and "yes"
}
#[test]
fn explicit_host_ip_is_refused() { /* ports ["0.0.0.0:80:80"] -> Err containing "riku.public" */ }
#[test]
fn volumes_short_and_long_forms() {
    // ["pgdata:/var/lib/postgresql/data", "./conf:/etc/app:ro", {type: bind, source: ./static, target: /srv, read_only: true}]
    // -> Volume("pgdata") rw, Bind(<app>/conf) ro, Bind(<app>/static) ro
}
#[test]
fn bind_mount_outside_app_is_refused() { /* "../other:/x" -> Err */ }
#[test]
fn healthcheck_forms() {
    // test ["CMD", "curl", "-f", "http://localhost/healthz"], interval "10s", retries 3 -> Exec([...]), Some(10s), Some(3)
    // test "pg_isready" -> Shell("pg_isready"); disable: true -> Disabled
}
#[test]
fn durations_combine_units() { assert_eq!(parse_duration("1m30s").unwrap(), Duration::from_secs(90)); }
#[test]
fn stop_grace_defaults_to_thirty_seconds() { assert_eq!(svc.stop_grace, Duration::from_secs(30)); }
#[test]
fn environment_wins_over_env_file() { /* env_file KEY=a, environment KEY=b -> "b" */ }
#[test]
fn env_file_outside_app_is_refused() { /* env_file "../../envs/shop/ENV" -> Err */ }
#[test]
fn completed_condition_is_refused() { /* depends_on {db: {condition: service_completed_successfully}} -> Err */ }
```

- [ ] **Step 2:** fails. **Step 3:** implement, one file per concern as listed. **Step 4:** pass; gate.
- [ ] **Step 5:** Flip B3. Commit `Parse compose service fields`

### Task B4: Project rules: order, routing, scale, replacement, volume names

**Files:**
- Create: `crates/riku-container/src/compose/{order.rs,order_tests.rs,rules.rs,rules_tests.rs,project.rs,project_tests.rs}`
- Modify: `compose/mod.rs`

**Interfaces:**
- Consumes: `read_raw`, `parse_service`, `ServiceDef`, `labels`.
- Produces:
  - `pub struct ComposeProject { pub services: Vec<ServiceDef>, pub volumes: BTreeMap<String, VolumeDecl>, pub project_name: String, pub warnings: Vec<String> }` (`services` in start order).
  - `pub enum VolumeDecl { Managed, Named(String), External(Option<String>) }`
  - `pub fn read_project(app_path: &Path, compose: &ComposeFile, app_env: &BTreeMap<String, String>) -> anyhow::Result<ComposeProject>`
  - `pub fn single_image_project(app_path: &Path, dockerfile: &Path) -> ComposeProject` (one service `app`, build context `.`, that dockerfile, no ports)
  - `pub fn existing_volume_name(project: &ComposeProject, key: &str) -> Option<String>`
  - `pub struct RoutedPort { pub service: ServiceName, pub container_port: u16 }`, `pub fn routed_http(project: &ComposeProject) -> anyhow::Result<Option<RoutedPort>>`
  - `pub fn check_scale(project: &ComposeProject, service: &ServiceName, copies: u32) -> anyhow::Result<()>`
  - `pub enum ReplaceOrder { Overlap, StopFirst }`, `pub fn replace_order(project: &ComposeProject, service: &ServiceName) -> ReplaceOrder`

Rules:
- Start order: dependencies first, ties by name; a cycle or an unknown dependency is refused naming the services.
- Project name (for existing volume names only): `COMPOSE_PROJECT_NAME` in `app_env`, else top level `name`, else the name of the directory holding the compose file; normalised the way compose does (lowercase, characters outside `a-z0-9_-` dropped). Verify the normalisation against the compose specification text before implementing and cite it in the doc comment.
- `existing_volume_name`: `Managed` gives `<project>_<key>`, `Named(n)` gives `n`, `External(Some(n))` gives `n`, `External(None)` gives `key`; `None` for an undeclared key.
- `routed_http`: services labelled `riku.http` (more than one is refused); the label value must equal a published host port of that service and selects that mapping's container port. With no label: no service publishes a port gives `None`; exactly one service with exactly one published port gives that port; any other case is refused with `several published ports (web: 8080, db: 5432); add the label riku.http: "<port>" to the service that serves the app's domains`.
- `check_scale` with `copies > 1`: refused when any mount is writable (`service 'db' mounts writable storage (volume pgdata at /var/lib/postgresql/data); storage services cannot scale`), and when the service publishes any port other than its routed HTTP port (`service 'db' publishes port 5432, which is not routed HTTP; only routed HTTP services can scale`). `copies == 0` refused.
- `replace_order`: `Overlap` only for the routed HTTP service with no writable mount; every other service `StopFirst`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn services_start_after_their_dependencies() { /* web depends_on db, worker depends_on web -> ["db", "web", "worker"] */ }
#[test]
fn dependency_cycle_is_refused() { /* web<->worker -> Err containing "web" and "worker" */ }
#[test]
fn existing_volumes_keep_compose_names() {
    // app dir ".../shop", volumes {pgdata: null, media: {name: shop_media}, shared: {external: true}}
    assert_eq!(existing_volume_name(&p, "pgdata").as_deref(), Some("shop_pgdata"));
    assert_eq!(existing_volume_name(&p, "media").as_deref(), Some("shop_media"));
    assert_eq!(existing_volume_name(&p, "shared").as_deref(), Some("shared"));
}
#[test]
fn project_name_follows_compose_file_directory() { /* COMPOSE_FILE "deploy/compose.yml" -> "deploy_pgdata" */ }
#[test]
fn project_name_env_wins() { /* app_env COMPOSE_PROJECT_NAME=Shop.Prod -> "shopprod_pgdata" */ }
#[test]
fn single_published_port_is_routed() { /* web "8080:80", worker none -> RoutedPort { web, 80 } */ }
#[test]
fn ambiguous_routing_without_label_is_refused() {
    // web "8080:80", db "5432:5432" -> Err containing "riku.http"
}
#[test]
fn http_label_picks_the_routed_port() { /* web labels riku.http "8080", ports ["8080:80", "9090:9090"] -> 80 */ }
#[test]
fn storage_service_cannot_scale() {
    let err = check_scale(&p, &db, 2).unwrap_err().to_string();
    assert!(err.contains("storage services cannot scale"), "{err}");
}
#[test]
fn read_only_mount_does_not_block_scaling() { /* web with "./conf:/etc/app:ro" scales to 3 */ }
#[test]
fn non_http_port_blocks_scaling() { /* web routed 80, also "9000:9000" -> Err containing "9000" */ }
#[test]
fn only_routed_web_overlaps() {
    // web Overlap, worker StopFirst, db (writable volume) StopFirst
}
#[test]
fn dockerfile_app_is_one_service_named_app() { /* services[0].name == "app", build.dockerfile == "Dockerfile" */ }
```

- [ ] **Step 2:** fails. **Step 3:** implement. **Step 4:** pass; gate.
- [ ] **Step 5:** Flip B4. Commit `Derive routing, scale and volume rules`

---

## Track C: routing plan

### Task C1: RoutePlan and Caddy rendering

**Files:**
- Create: `crates/riku-util/src/route_plan.rs`
- Modify: `crates/riku-util/src/lib.rs`, `crates/riku-caddy/src/{lib.rs,site.rs,site_tests.rs}`, `crates/riku-deploy/src/router.rs`, `crates/riku-deploy/src/router/caddy.rs`, `caddy_tests.rs`, `router_tests.rs`

**Interfaces:**
- Consumes: `Backend` (Task 0), `Domain`, `parse_domains`, `https_only`.
- Produces:
  - `riku_util::route_plan::{RoutePlan, HttpsPolicy}`: `pub struct RoutePlan { pub domains: Vec<Domain>, pub https: HttpsPolicy, pub backends: Vec<Backend> }`; `HttpsPolicy` moved from riku-caddy unchanged, re-exported by `riku_caddy`.
  - `riku_caddy::render_site(app: &str, plan: &RoutePlan) -> anyhow::Result<String>`, `write_site(app: &str, plan: &RoutePlan, paths: &RikuPaths) -> anyhow::Result<()>`. `SiteRequest` is removed.
  - In riku-deploy `router.rs`: `fn env_backends(env: &HashMap<String, String>) -> Vec<Backend>` (`vec![Backend::Tcp(upstream_port_from_env(env))]`) and `fn route_plan(env: &HashMap<String, String>) -> anyhow::Result<RoutePlan>`; phase 3 replaces `env_backends` with the supervisor's copies.

Caddy renders every backend on one line, `reverse_proxy 127.0.0.1:5001 127.0.0.1:5002`; with one backend the file is byte for byte today's. An empty backend list or a `Unix` backend is refused (`Caddy proxies TCP only`).

- [ ] **Step 1: Write the failing tests.** Update the two existing `site_tests` to build a `RoutePlan` with `backends: vec![Backend::Tcp(5000)]` and keep their expected strings unchanged. Add:

```rust
#[test]
fn every_backend_is_an_upstream() {
    let plan = plan(HttpsPolicy::Only, vec![Backend::Tcp(5001), Backend::Tcp(5002)]);
    assert!(render_site("shop", &plan).unwrap().contains("\treverse_proxy 127.0.0.1:5001 127.0.0.1:5002\n"));
}
#[test]
fn socket_backend_is_refused() {
    let err = render_site("shop", &plan(HttpsPolicy::Only, vec![Backend::Unix("/x.sock".into())])).unwrap_err();
    assert!(err.to_string().contains("TCP only"), "{err}");
}
```

- [ ] **Step 2:** fails to compile. **Step 3:** implement; `configure_caddy` builds the plan through `route_plan` and removes the site when `domains` is empty, as today. **Step 4:** `cargo test -q -p riku-caddy -p riku-deploy router` passes; gate.
- [ ] **Step 5:** Flip C1. Commit `Render Caddy sites from a routing plan`

### Task C2: nginx upstream from backends

**Files:**
- Create: `templates/nginx_upstream.conf.tera`, `crates/riku-nginx/src/backends.rs`, `backends_tests.rs`
- Modify: `templates/nginx_portmap.conf.tera`, `templates/nginx_https_only.conf.tera:1-3`, `crates/riku-nginx/src/{lib.rs,template.rs,template_tests.rs,context.rs,tests.rs}`, `crates/riku-deploy/src/router.rs`

**Interfaces:**
- Consumes: `Backend`.
- Produces:
  - `riku_nginx::default_backends(app: &str, env: &HashMap<String, String>, paths: &RikuPaths) -> Vec<Backend>`: `RIKU_NGINX_PORTMAP` set gives `Tcp(RIKU_UPSTREAM_PORT, then PORT, default 8080)`; otherwise `Unix(RIKU_NGINX_SOCKET or <nginx_root>/<app>.sock)` (the values `context.rs` uses today; `context.rs` calls the same helpers so the default lives in one place).
  - `riku_nginx::generate_nginx_config(app: &str, app_path: &Path, env: &HashMap<String, String>, backends: &[Backend], paths: &RikuPaths) -> anyhow::Result<()>`; empty `backends` is refused.
  - `pub(crate) fn render_nginx_config(app, app_path, env, backends, paths) -> anyhow::Result<String>` split out of `generate_nginx_config_from_template` for tests.
  - Template context key `RIKU_BACKENDS: Vec<String>` (each `Backend` displayed).

nginx keeps reading domains and TLS from the sanitised ENV as today, not from `RoutePlan.domains`: `Domain` validation is stricter than nginx sanitisation and switching would refuse apps that deploy now. The partial:

```
upstream {{ APP }}_UPSTREAM {
{% for backend in RIKU_BACKENDS %}  server {{ backend }};
{% endfor %}}
```

`nginx_portmap.conf.tera` includes it at the top and proxies to `http://{{ APP }}_UPSTREAM`. `nginx_https_only.conf.tera` replaces its own upstream block (which writes a socket path without `unix:`) with the include. Locations in `nginx_common.conf.tera` are not changed.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn portmap_proxies_to_every_backend() {
    // env RIKU_NGINX_PORTMAP=true; backends [Tcp(5001), Tcp(5002)]
    // rendered contains "upstream myapp_UPSTREAM {\n  server 127.0.0.1:5001;\n  server 127.0.0.1:5002;\n}" and "proxy_pass http://myapp_UPSTREAM;"
}
#[test]
fn default_backend_follows_portmap_and_socket() {
    // PORTMAP + PORT=5000 -> [Tcp(5000)]; no PORTMAP -> [Unix(<nginx_root>/myapp.sock)]
}
#[test]
fn empty_backends_are_refused() { /* generate_nginx_config(.., &[], ..) -> Err */ }
#[test]
fn https_only_upstream_names_a_unix_socket() {
    // render the https_only template with a hand built Tera context (APP, RIKU_BACKENDS ["unix:/s.sock"], no SSL step)
    // expect "server unix:/s.sock;"
}
```

- [ ] **Step 2:** fails. **Step 3:** implement; update every `generate_nginx_config` caller (riku-deploy router Nginx arm passes `default_backends`, nginx `tests.rs` passes the same).
- [ ] **Step 4:** Check before committing: render the HTTPS only template for an env with `RIKU_NGINX_PORTMAP=true` and `RIKU_HTTPS_ONLY=true` and record where its `location /` proxies. Today `select_template` picks HTTPS only before portmap and `nginx_common.conf.tera` proxies to the unix socket, so an HTTPS only Procfile web app may not reach its TCP port. Do not change that here; report the result to the owner in the task report and add it to the ROADMAP "Known open problems" list if confirmed.
- [ ] **Step 5:** Tests pass; gate. Flip C2. Commit `Render nginx upstreams from backends`

### Task C3: Router plugin protocol 2

**Files:**
- Create: `crates/riku-bundle/src/router_section.rs`, `router_section_tests.rs`, `crates/riku-deploy/src/router/plugin.rs`, `plugin_tests.rs`
- Modify: `crates/riku-bundle/src/{lib.rs,manifest.rs}` (field only), `crates/riku-deploy/src/router.rs` (move the plugin arms into `router/plugin.rs`), `PLUGIN_PROTOCOL.md` §6.2, `CHANGELOG.md`

**Interfaces:**
- Consumes: `route_plan` (C1), `bundles::find_router`, `router::run_verb`.
- Produces:
  - `riku_bundle::ROUTER_PROTOCOL: u32 = 2`; `pub struct RouterSection { pub protocol: u32, #[serde(default)] pub graceful_reload: bool }`; `PluginManifest.router: Option<RouterSection>` (`#[serde(default)]`, TOML table `[router]`).
  - In `router/plugin.rs`: `fn configure_request(app: &str, plan: &RoutePlan) -> serde_json::Value`, `pub(super) fn configure_plugin(name, app, env, paths) -> Result<()>`, `pub(super) fn remove_plugin(name, app, paths) -> Result<()>` (unchanged behaviour).

Request on stdin for `configure` (exact shape):

```json
{"protocol":2,"app":"shop","domains":["shop.example.com"],"https":true,"backends":[{"kind":"tcp","host":"127.0.0.1","port":5001},{"kind":"unix","path":"/srv/riku/nginx/shop.sock"}]}
```

A router plugin whose manifest has no `[router]` table or a `protocol` other than 2 is refused before any verb runs: `router plugin '<name>' speaks router protocol <n or 1>; this riku needs protocol 2. Update the plugin (PLUGIN_PROTOCOL.md section 6.2)`. `unconfigure` keeps its best effort behaviour for any protocol. Domains now pass through `parse_domains`, so an invalid `RIKU_DOMAINS` fails a plugin router deploy the way it already fails a Caddy deploy; say so in CHANGELOG. `graceful_reload` is read by phase 3 draining; it is documented now.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn router_section_parses() {
    // manifest text with "[router]\nprotocol = 2\ngraceful_reload = true" -> Some(RouterSection { protocol: 2, graceful_reload: true })
}
#[test]
fn configure_request_lists_every_backend() {
    assert_eq!(configure_request("shop", &plan), serde_json::json!({ /* the exact JSON above */ }));
}
#[test]
fn old_router_plugin_is_refused_before_running() {
    // bundle "edge-router" with type router, no [router] table, entry script touches <tmp>/ran
    let err = configure_plugin("edge-router", "shop", &env, &paths).unwrap_err().to_string();
    assert!(err.contains("router protocol 1"), "{err}");
    assert!(!tmp.path().join("ran").exists(), "plugin ran before the protocol check");
}
```

- [ ] **Step 2:** fails. **Step 3:** implement; update §6.2 (verb table, request example, `[router]` table, graceful reload duty). **Step 4:** pass; gate.
- [ ] **Step 5:** Flip C3. Commit `Send backend lists to router plugins`

---

## Track D: riku file sync (`crates/riku-sync`)

Security model for the whole crate (goes in its `//!` doc): the receiving side treats the manifest and the tar stream as hostile. Every path is a `RelPath`; files are written only into a fresh staging directory, never through a symlink, never more bytes than the manifest states, and the live directory is replaced only after every file is verified.

### Task D1: Crate, wire types and manifest scan

**Files:**
- Create: `crates/riku-sync/{Cargo.toml,src/lib.rs,src/wire.rs,src/wire_tests.rs,src/scan.rs,src/scan_tests.rs}`
- Modify: root `Cargo.toml` (member), `.claude/CLAUDE.md` (crate list line after riku-util)

**Interfaces:**
- Produces:
  - `pub struct RelPath(String)`: `TryFrom<&str>` and serde `try_from = "String"`; refuses empty, leading `/`, empty, `.` or `..` components, `\`, NUL; `as_str()`, `to_path(&self) -> PathBuf`, `parent(&self) -> Option<RelPath>`.
  - `pub enum EntryKind { File { size: u64, sha256: String, mode: u32 }, Symlink { target: String }, Dir }` (serde tag `kind`).
  - `pub struct Entry { pub path: RelPath, pub kind: EntryKind }`, `pub struct Manifest { pub entries: Vec<Entry> }` (sorted by path), `pub struct TransferPlan { pub need: Vec<RelPath>, pub delete: Vec<RelPath> }`.
  - `pub fn scan(root: &Path) -> anyhow::Result<Manifest>`: never follows symlinks; `mode` is `permissions & 0o777`; sockets, fifos and devices are refused naming the path; a non UTF-8 name is refused naming its lossy form.

Dependencies: anyhow, serde, serde_json, sha2 (0.11, already in the lockfile), hex; dev tempfile. `tar` and `zstd` come in D2.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn rel_path_refuses_escapes() {
    for bad in ["", "/etc/passwd", "../x", "a/../b", "a//b", "a/./b", "a\\b"] {
        assert!(RelPath::try_from(bad).is_err(), "accepted {bad:?}");
    }
}
#[test]
fn manifest_json_with_escape_fails_to_parse() {
    let raw = r#"{"entries":[{"path":"../x","kind":"dir"}]}"#;
    assert!(serde_json::from_str::<Manifest>(raw).is_err());
}
#[test]
fn scan_records_files_dirs_and_symlinks() {
    // tree: app.py = "hello\n", bin/run (0755), venv/bin/python -> /usr/bin/python3, empty dir logs/
    // app.py: size 6, sha256 "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03", mode 0o644
    // bin/run mode 0o755; venv/bin/python Symlink { target: "/usr/bin/python3" }; logs Dir
    // entries sorted by path
}
```

- [ ] **Step 2:** fails. **Step 3:** implement. **Step 4:** pass; gate.
- [ ] **Step 5:** Flip D1. Commit `Add riku-sync manifest scan`

### Task D2: Transfer plan, bundle and verified apply

**Files:**
- Create: `crates/riku-sync/src/{plan.rs,plan_tests.rs,bundle.rs,bundle_tests.rs,apply.rs,apply_tests.rs}`
- Modify: `crates/riku-sync/{Cargo.toml,src/lib.rs}`

**Interfaces:**
- Consumes: D1 types and `scan`.
- Produces:
  - `pub fn plan_transfer(live: &Path, incoming: &Manifest) -> anyhow::Result<TransferPlan>`: a missing `live` needs every file; `need` holds incoming files that are absent or differ in size or hash or are not a file in live; `delete` holds live paths absent from incoming. A mode only change is not in `need` (apply sets modes from the manifest).
  - `pub fn write_bundle(src: &Path, need: &[RelPath], out: impl Write) -> anyhow::Result<()>`: one zstd compressed tar (level 3) of regular files only.
  - `pub fn apply(live: &Path, manifest: &Manifest, plan: &TransferPlan, bundle: impl Read) -> anyhow::Result<()>`

Dependencies: `tar`, `zstd` (check `cargo info` for current versions; `zstd` builds C code, so confirm `cargo build --release --target aarch64-unknown-linux-gnu` works with the cross setup in `.github/workflows/release.yml`, or say it was not verified locally).

Apply algorithm (the tests do not fix the order, so it is given here):
1. Staging is `<live>.sync-new` beside `live`; a leftover one is removed first.
2. Create every `Dir` entry. Copy every `File` entry not in `need` from `live` (copy, not hard link).
3. Read the tar: each entry must be a regular file whose path is a `RelPath` in `need`, seen once, whose parent chain is `Dir` entries in the manifest; write with `create_new`, reading at most `size + 1` bytes and refusing more than `size`.
4. Create every `Symlink` entry.
5. Verify every `File` entry in staging (size and sha256), then set its mode. Every `need` path must have arrived.
6. Rename `live` to `<live>.sync-old`, staging to `live`, remove `.sync-old`; if the second rename fails, rename `.sync-old` back.
7. On any error before step 6, remove staging and leave `live` untouched.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn plan_needs_only_changed_files() {
    // live: a.txt "1", b.txt "2", old.txt; incoming: a.txt "1", b.txt "3", c.txt "4"
    // need ["b.txt", "c.txt"], delete ["old.txt"]
}
#[test]
fn apply_makes_live_match_the_source() {
    // round trip src -> bundle -> apply; scan(live) == scan(src); old.txt gone; no .sync-new or .sync-old left
}
#[test]
fn unrequested_path_in_bundle_is_refused() { /* bundle carries "extra.txt" not in need -> Err; live unchanged */ }
#[test]
fn file_under_symlinked_dir_is_refused() {
    // manifest: "lib" Symlink to "/etc", need ["lib/passwd"] -> Err; nothing written outside staging
}
#[test]
fn oversized_file_is_refused() { /* manifest size 3, tar entry 10 bytes -> Err containing the path; live unchanged */ }
#[test]
fn hash_mismatch_is_refused() { /* same size, different bytes -> Err; live unchanged */ }
```

"live unchanged" means `scan(live)` before equals after, and staging does not exist.

- [ ] **Step 2:** fails. **Step 3:** implement. **Step 4:** pass; gate.
- [ ] **Step 5:** Flip D2. Commit `Plan and apply verified sync bundles`

### Task D3: The sync conversation over a byte stream

**Files:**
- Create: `crates/riku-sync/src/{session.rs,session_tests.rs}`
- Modify: `crates/riku-sync/src/lib.rs`

**Interfaces:**
- Consumes: D1 and D2.
- Produces:
  - `pub const SYNC_PROTOCOL: u32 = 1;`, `pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;`
  - `pub struct SendReport { pub sent: Vec<RelPath>, pub deleted: Vec<RelPath>, pub bytes: u64 }`
  - `pub fn send(src: &Path, from_server: impl BufRead, to_server: impl Write) -> anyhow::Result<SendReport>`
  - `pub fn receive(live: &Path, from_client: impl BufRead, to_client: impl Write) -> anyhow::Result<()>`

Conversation (one JSON object per line, then the bundle): client `{"sync":1,"manifest":{...}}`; server `{"need":[...],"delete":[...]}`; client sends the zstd tar; server applies and answers `{"ok":true}` or `{"error":"<message>"}`. The server refuses another `sync` value and any line longer than `MAX_LINE_BYTES` before parsing it. `send` returns the server's error text as its error. Phase 3 runs `receive` behind an ssh command; nothing calls it in phase 2.

- [ ] **Step 1: Write the failing tests** (client and server on two threads joined by a `UnixStream::pair()` per direction)

```rust
#[test]
fn second_sync_moves_only_the_changed_file() {
    // first send: sent == all files; edit app.py, delete old.txt; second send:
    assert_eq!(report.sent, vec![rel("app.py")]);
    assert_eq!(report.deleted, vec![rel("old.txt")]);
}
#[test]
fn cut_stream_leaves_live_untouched() {
    // client writes the manifest line, reads the plan, writes half the bundle, drops the socket
    // receive returns Err; scan(live) unchanged; no .sync-new left
}
#[test]
fn unknown_protocol_is_refused() { /* {"sync":2,...} -> server answers {"error":...} containing "sync protocol 2" */ }
#[test]
fn overlong_manifest_line_is_refused() { /* MAX_LINE_BYTES + 1 bytes without newline -> Err before allocation beyond the cap */ }
```

- [ ] **Step 2:** fails. **Step 3:** implement. **Step 4:** pass; gate; run `cargo test -q -p riku-sync session` 20 times with default threads and with `--test-threads 1`.
- [ ] **Step 5:** Flip D3. Commit `Run the sync conversation over streams`

---

## Out of scope (phase 3)

Supervisor running containers and counting restarts, per copy port allocation, health wait limits and the waits themselves, rolling replacement and draining, the synchronous confirmed router reload, compose migration (stopping the old stack, fallback restart, removing the old worker config, `riku doctor` check), retiring the container plugin's start command and moving its registry login into the driver, image watch on the new model, state API, dashboard rows and engine header, `riku ps --scale`, `riku logs <app> <service>`, `rk deploy --build`, wiring `riku-sync` behind ssh and measuring its binary size, WSGI one socket per copy, mapping compose `restart` to the supervisor policy, engine call time limits.
