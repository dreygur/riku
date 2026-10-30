# Remote CLI and Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Local `riku` manages named remote riku servers over the system `ssh`, opens a per session remote dashboard, groups its help by where commands run, and every dependency is on its latest stable major.

**Architecture:** The server parses `SSH_ORIGINAL_COMMAND` itself so quoted arguments survive. The laptop keeps a TOML server list; a pure resolver picks a target (flag, git remote, default, local); forwarding rebuilds argv and execs `ssh`. The dashboard client drives one shared OpenSSH connection (control socket) to start a remote dashboard on port 0 and forward to it.

**Tech Stack:** Rust 2021 workspace, clap 4 builder plus derive, `toml`, `shlex`, `rand`, `nix`, `tempfile`, system `ssh` and `git`.

**Spec:** `docs/superpowers/specs/2026-10-29-remote-cli-design.md`

## Global Constraints

- Project rules in `.claude/CLAUDE.md` apply: no `?` in new or touched functions (explicit `match`/`return Err`), tests in sibling `_tests.rs` files via `#[path]`, files under about 200 lines, doc comments on public items, no emoji, no em dashes, no AI attribution, commits via `git commit -F <file> --no-verify`.
- No ssh or git library. Run the system `ssh` and `git`.
- Deploy keys keep `no-port-forwarding`.
- `shlex` is the only new direct dependency (already in `Cargo.lock` transitively).
- Server list path: `${XDG_CONFIG_HOME:-$HOME/.config}/riku/servers.toml`, mode 600. Never `dirs::config_dir()` (it is `~/Library/Application Support` on macOS).
- Target precedence: `-s/--server`, then git remote match, then default, then local. `--local` forces local.
- Host matching is exact on `user@host`. No DNS, no ssh config alias resolution.
- Every forwarded command first prints `[*] <name> (<address>)` to stderr. No new confirmation prompts.
- A leading `riku` word in `SSH_ORIGINAL_COMMAND` is dropped.
- Dashboard: fresh token per session, never in an argument list; per session remote process; local port 8088 else any free port.
- Help group order: Local, Server, Forwarded.

## Review Focus

1. Recursion on the server: a forced command invocation (`SSH_ORIGINAL_COMMAND` set) must never forward, even if that account has a `servers.toml` with a default. Expect local execution. Test in Task 6.
2. User arguments that look like riku flags: `riku -s prod run myapp grep -s x` must forward `run myapp grep -s x` intact. Only the tokens clap consumed for `-s/--local` are removed. Test in Task 6.
3. Unbalanced quotes in `SSH_ORIGINAL_COMMAND` (`run app echo "a`): expect a clear error and nonzero exit, never a partial command. Test in Task 2.
4. Long `TMPDIR` on macOS overflowing the 104 byte unix socket limit for the control socket. Expect `-S <dir>/%C` (ssh hashes the name), so it works with any TMPDIR. Test in Task 8.
5. Version skew: a new laptop forwarding `--app-hint` to an old server fails with clap's unknown argument error. Expected and accepted, but the forwarded command path must only add `--app-hint` when a hint exists. Test in Task 6.

---

## File Structure

```
crates/riku-cli/src/
  ssh_command.rs            server side: SSH_ORIGINAL_COMMAND to argv
  ssh_command_tests.rs
  placement.rs              Placement enum, name to placement table
  placement_tests.rs
  help.rs                   grouped help template
  help_tests.rs
  remote/
    mod.rs                  module docs, re-exports
    types.rs                ServerName, SshAddress, Server, ServerList
    types_tests.rs
    store.rs                servers.toml read/write (repository)
    store_tests.rs
    git_remote.rs           parse `git remote -v`, run git
    git_remote_tests.rs
    target.rs               Target, resolve() (pure service)
    target_tests.rs
    forward.rs              argv rebuild, ssh invocation
    forward_tests.rs
    server_cmd.rs           `riku server ...` handlers (provider)
    dashboard/
      mod.rs                session orchestration
      ssh_master.rs         control socket start, check, forward, exit
      remote_script.rs      the remote sh script and stdin payload
      remote_script_tests.rs
      sudo.rs               probe, local no echo password prompt
      browser.rs            open URL per platform
crates/riku-util/src/
  authorized_keys.rs        forced command migration (pure + atomic write)
  authorized_keys_tests.rs
```

Modified: `crates/riku-util/src/ssh_keys.rs`, `crates/riku-cli/src/cli.rs`, `crates/riku-cli/src/cmds.rs`, `crates/riku-cli/src/routing.rs`, `crates/riku-cli/src/doctor/{mod.rs,checks.rs}`, `crates/riku-cli/src/setup/deploy_keys.rs`, `crates/riku-dashboard/src/lib.rs`, `crates/riku/src/main.rs`, `tests/stress/container/{Dockerfile,entrypoint.sh}` (comments).

---

### Task 1: Bump every dependency to its latest stable major

One commit per ecosystem so a breakage is attributable.

**Files:**
- Modify: `Cargo.toml`, `crates/*/Cargo.toml`, `Cargo.lock`, and any Rust source the bump breaks
- Modify: `dashboard/package.json` plus lockfile
- Modify: `examples/golang/go.mod`, `go.sum`; `tests/demo/apps/hello-go/go.mod`
- Modify: `examples/nodejs*/package.json`, `tests/demo/apps/hello-node/package.json`
- Modify: every `requirements.txt` listed by `find . -name requirements.txt -not -path ./target/*`, `tests/demo/apps/hello-ruby/Gemfile`

- [ ] **Step 1: Record baseline**

Run: `cargo test -q 2>&1 | grep "^test result"` and note totals (last known: 677 passed, 0 failed, 2 ignored).

- [ ] **Step 2: Bump Rust**

Run: `cargo install cargo-edit --locked && cargo upgrade --incompatible allow && cargo update`
(`cargo update --breaking` is unstable on cargo 1.98, so cargo-edit is used.) Fix every compile error at its root, per rule 7. No `#[allow]` to hide new warnings.

- [ ] **Step 3: Verify Rust**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -q`
Expected: clean, test totals equal to baseline.

- [ ] **Step 4: Commit** `Bump Rust dependencies to latest majors`

- [ ] **Step 5: Bump the dashboard**

Run in `dashboard/`: update every dependency in `package.json` to its latest stable major (`npx npm-check-updates -u`), then `nub install && nub run build`.
Expected: build succeeds. Commit `Bump dashboard dependencies to latest majors`.

- [ ] **Step 6: Bump Go example and demo app**

Run in each Go module: `go get -u ./... && go mod tidy && go build ./...`
Verify the Dependabot alerts are covered: `gh api repos/dreygur/riku/dependabot/alerts --jq '.[]|select(.state=="open")|[.dependency.package.name,.security_vulnerability.first_patched_version.identifier]|@tsv'` and check `go list -m all` in `examples/golang` is at or above every patched version. Commit `Bump Go example dependencies`.

- [ ] **Step 7: Bump Node, Python and Ruby fixtures**

Node: `npx npm-check-updates -u && npm install` per package. Python: set each pin to the latest release from `pip index versions <pkg>`. Ruby: `bundle update` if bundler is present, otherwise raise pins by hand. Then run `./tests/deploy-smoke/test-all.sh` if a local riku host is available; if not, say so in the commit body. Commit `Bump example and demo app dependencies`.

---

### Task 2: Server side argv from SSH_ORIGINAL_COMMAND

**Files:**
- Create: `crates/riku-cli/src/ssh_command.rs`, `crates/riku-cli/src/ssh_command_tests.rs`
- Modify: `crates/riku-cli/src/lib.rs` (add module), `crates/riku/src/main.rs:35-40`, root `Cargo.toml` and `crates/riku-cli/Cargo.toml` (add `shlex`)

**Interfaces:**
- Produces: `pub fn process_argv() -> anyhow::Result<Vec<String>>` and `pub fn split_original(original: &str) -> anyhow::Result<Vec<String>>` in `riku_cli::ssh_command`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn keeps_quoted_argument_whole() {
    let argv = split_original(r#"run myapp echo "a b""#).unwrap();
    assert_eq!(argv, ["run", "myapp", "echo", "a b"]);
}

#[test]
fn drops_leading_riku_word() {
    assert_eq!(split_original("riku ps myapp").unwrap(), ["ps", "myapp"]);
}

#[test]
fn git_push_command_loses_its_quotes() {
    assert_eq!(split_original("git-receive-pack 'myapp'").unwrap(), ["git-receive-pack", "myapp"]);
}

#[test]
fn does_not_expand_globs_or_variables() {
    assert_eq!(split_original("run app ls * $HOME").unwrap(), ["run", "app", "ls", "*", "$HOME"]);
}

#[test]
fn rejects_unbalanced_quote() {
    let err = split_original(r#"run app echo "a"#).unwrap_err();
    assert!(err.to_string().contains("unbalanced"), "got: {err}");
}

#[test]
fn no_subcommand_is_named_riku() {
    use clap::CommandFactory;
    let names: Vec<String> = crate::cli::Cli::command()
        .get_subcommands().map(|c| c.get_name().to_string()).collect();
    assert!(!names.iter().any(|n| n == "riku"), "dropping a leading riku word would shadow it");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riku-cli ssh_command`
Expected: FAIL, `split_original` not found.

- [ ] **Step 3: Implement**

`split_original` uses `shlex::split`; `None` becomes an error containing `unbalanced quotes in SSH command`. `process_argv`: when `std::env::args().count() == 1` and `SSH_ORIGINAL_COMMAND` is set and nonempty, return `["riku", ..split_original(..)]`; otherwise `std::env::args().collect()`. In `main.rs`, replace `Cli::parse()` with parsing from `process_argv()` (Task 4 replaces this call again with the grouped help command, keep the argv source).

- [ ] **Step 4: Verify**

Run: `cargo test -p riku-cli ssh_command && cargo test -q`
Expected: PASS, totals baseline plus 6.

- [ ] **Step 5: Commit** `Parse SSH_ORIGINAL_COMMAND with shell quoting`

---

### Task 3: New forced command and key migration

**Files:**
- Create: `crates/riku-util/src/authorized_keys.rs`, `crates/riku-util/src/authorized_keys_tests.rs`
- Modify: `crates/riku-util/src/ssh_keys.rs:26` (drop ` $SSH_ORIGINAL_COMMAND`), `crates/riku-util/src/lib.rs`, `crates/riku-cli/src/doctor/checks.rs`, `crates/riku-cli/src/doctor/mod.rs` (check list, `--fix`, module doc: read only unless `--fix`), `crates/riku-cli/src/cli.rs:241` (`Doctor { #[arg(long)] fix: bool }`), `crates/riku/src/main.rs` (doctor arm), `crates/riku-cli/src/setup/deploy_keys.rs` (migrate after adding keys), stress container comments

**Interfaces:**
- Produces: `pub fn migrate_line(line: &str) -> Option<String>`, `pub fn migrate_text(text: &str) -> (String, usize)`, `pub fn migrate_file(path: &Path) -> anyhow::Result<usize>` in `riku_util::authorized_keys`.

- [ ] **Step 1: Write failing tests**

```rust
const OLD: &str = r#"command="FINGERPRINT=SHA256:ab NAME=default /home/riku/.local/bin/riku $SSH_ORIGINAL_COMMAND",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding ssh-ed25519 AAAA me@laptop"#;
const NEW: &str = r#"command="FINGERPRINT=SHA256:ab NAME=default /home/riku/.local/bin/riku",no-agent-forwarding,no-user-rc,no-X11-forwarding,no-port-forwarding ssh-ed25519 AAAA me@laptop"#;

#[test]
fn rewrites_old_forced_command() { assert_eq!(migrate_line(OLD).as_deref(), Some(NEW)); }

#[test]
fn leaves_new_and_foreign_lines_alone() {
    assert_eq!(migrate_line(NEW), None);
    assert_eq!(migrate_line("ssh-ed25519 AAAA admin@host"), None);
    assert_eq!(migrate_line(r#"command="riku agent --scope staging",no-port-forwarding ssh-rsa AAAA"#), None);
}

#[test]
fn migrate_file_keeps_mode_600_and_counts() {
    // temp dir, file with OLD + foreign line, mode 600
    // assert returned 1, contents == NEW + foreign line, mode 0o600, rerun returns 0
}

#[test]
fn new_keys_get_plain_forced_command() {
    // reuse ssh_keys.rs with_home helper; after setup_authorized_keys
    assert!(!content.contains("$SSH_ORIGINAL_COMMAND"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riku-util authorized_keys`
Expected: FAIL, module missing.

- [ ] **Step 3: Implement**

`migrate_line` acts only when the line contains the `FINGERPRINT=` marker and its `command="..."` value ends in ` $SSH_ORIGINAL_COMMAND"`; it removes that suffix and nothing else. `migrate_file` writes a sibling temp file with mode 600 and renames it over the original only when the count is nonzero. Doctor gains a `forced_commands` check: Warn `N deploy keys use the old forced command, run: riku doctor --fix` when any line migrates, Ok otherwise. `--fix` runs `migrate_file` and prints the count. `deploy_keys.rs` calls `migrate_file` after writing keys. Move the `RIKU_KEY_MARKER` constant to `authorized_keys.rs` and use it from doctor.

- [ ] **Step 4: Verify**

Run: `cargo test -q && cargo clippy --all-targets -- -D warnings`
Expected: PASS. Then on edge, with the new binary installed: `riku doctor` shows the warning, `riku doctor --fix` reports the count, `git push` and `ssh riku@edge.example 'run myapp echo "a b"'` print `a b`. If edge is unreachable, record that as not verified.

- [ ] **Step 5: Commit** `Let riku parse the SSH command itself`

Body must say: install the new binary before running `--fix`, an old binary with migrated keys cannot run any command.

---

### Task 4: Command placement and grouped help

**Files:**
- Create: `crates/riku-cli/src/placement.rs`, `placement_tests.rs`, `help.rs`, `help_tests.rs`
- Modify: `crates/riku-cli/src/lib.rs`, `crates/riku/src/main.rs`

**Interfaces:**
- Produces: `pub enum Placement { Local, Server, Forwarded }`, `pub fn placement_of(name: &str) -> Option<Placement>`, `pub fn command(forward_heading: &str) -> clap::Command` (in `help.rs`, returns `Cli::command()` with the grouped template).

Table (spec, plus designer calls): Local = server, quickstart, container, plugin. Server = init, supervisor, dashboard, git-hook, git-receive-pack, git-upload-pack, scp, plugin-emit, ns-shim, dump-state. Forwarded = apps, ps, logs, stats, config, deploy, restart, stop, destroy, rollback, run, backup, restore, addon, doctor, update, agent, install-plugins, plugins, hook. (`server` is added in Task 7; keep it in the table now.)

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn every_subcommand_has_a_placement() {
    let missing: Vec<String> = crate::cli::Cli::command().get_subcommands()
        .map(|c| c.get_name().to_string())
        .filter(|n| n != "help" && placement_of(n).is_none()).collect();
    assert!(missing.is_empty(), "no placement for: {missing:?}");
}

#[test]
fn help_lists_groups_in_order_and_each_visible_command_once() {
    let text = command("Forwarded commands:").render_help().to_string();
    let (l, s, f) = (text.find("Local commands:").unwrap(), text.find("Server commands:").unwrap(), text.find("Forwarded commands").unwrap());
    assert!(l < s && s < f, "group order wrong:\n{text}");
    for name in ["apps", "logs", "init", "quickstart"] {
        assert_eq!(text.matches(&format!("\n  {name} ")).count(), 1, "{name} not listed exactly once:\n{text}");
    }
    assert!(!text.contains("git-hook"), "hidden command shown:\n{text}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riku-cli placement help`
Expected: FAIL.

- [ ] **Step 3: Implement**

`help.rs` builds the group text from `Cli::command().get_subcommands()` (skipping hidden ones, keeping each `get_about()` line, aligned like clap's own list) and sets it with `help_template("{about-with-newline}\n{usage-heading} {usage}\n\n<groups>\n{options}")`. Escape `{` and `}` in descriptions. `main.rs` parses with `help::command(..).get_matches_from(process_argv())` then `Cli::from_arg_matches`. The forwarded heading text comes from Task 6; until then pass `"Forwarded commands (run on the selected server, or here with --local):"`.

- [ ] **Step 4: Verify**

Run: `cargo test -q && cargo run -q -- -h`
Expected: PASS; help shows three groups in order.

- [ ] **Step 5: Commit** `Group help by where commands run`

---

### Task 5: Server list types and store

**Files:**
- Create: `crates/riku-cli/src/remote/{mod.rs,types.rs,types_tests.rs,store.rs,store_tests.rs}`
- Modify: `crates/riku-cli/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct ServerName(String)`, `impl TryFrom<&str>`: 1 to 32 chars of `[a-z0-9_-]`, first char alphanumeric.
  - `pub struct SshAddress(String)`, `impl TryFrom<&str>`: exactly one `@`, nonempty user and host, no whitespace, no leading `-`, only `[A-Za-z0-9._@:-]` plus `[` `]` for IPv6. Methods `user() -> &str`, `host() -> &str`, `as_str()`.
  - `pub struct Server { pub name: ServerName, pub address: SshAddress, pub admin: Option<SshAddress> }`
  - `pub struct ServerList { servers: Vec<Server>, default: Option<ServerName> }` with `get(&ServerName) -> Option<&Server>`, `default_server() -> Option<&Server>`, `add(Server) -> Result<()>` (duplicate name is an error), `remove(&ServerName) -> Result<()>` (clears default if it was default), `set_default(&ServerName) -> Result<()>`, `iter()`.
  - `store.rs`: `pub fn servers_path() -> anyhow::Result<PathBuf>`, `pub fn load(path: &Path) -> anyhow::Result<ServerList>` (missing file is an empty list), `pub fn save(path: &Path, list: &ServerList) -> anyhow::Result<()>` (create dir 700, temp file mode 600, rename). Serde uses plain strings in TOML and validates through `TryFrom` on load, so a hand edited bad entry fails with the file path and entry name.

- [ ] **Step 1: Write failing tests**

`rejects_address_starting_with_dash` (`-oProxyCommand=x@h`), `rejects_address_with_shell_metacharacters` (`riku@host;rm`), `accepts_ipv6_host` (`riku@[::1]`), `removing_default_clears_it`, `duplicate_name_is_rejected`, `round_trips_through_toml_with_mode_600` (temp dir, save, load, compare, `0o600`), `missing_file_loads_empty`, `bad_entry_names_file_and_server` (asserts both appear in the error text), `servers_path_prefers_xdg_config_home` (pass the env values in: make it `servers_path_from(xdg: Option<&str>, home: &str)` and test that, so no test mutates process env; per rule 20).

- [ ] **Step 2: Run to verify failure**, `cargo test -p riku-cli remote::`

- [ ] **Step 3: Implement** per Interfaces.

- [ ] **Step 4: Verify**, `cargo test -q`, PASS.

- [ ] **Step 5: Commit** `Add the local server list`

---

### Task 6: Target resolution and forwarding

**Files:**
- Create: `crates/riku-cli/src/remote/{git_remote.rs,git_remote_tests.rs,target.rs,target_tests.rs,forward.rs,forward_tests.rs}`
- Modify: `crates/riku-cli/src/cli.rs` (global args, optional app), `crates/riku-cli/src/cmds.rs`, `crates/riku-cli/src/routing.rs` (`build_plugin_args` takes `server: Option<&SshAddress>`), `crates/riku/src/main.rs`, `crates/riku-cli/src/help.rs` heading

**Interfaces:**
- Consumes: Task 5 types, Task 4 `placement_of`, Task 2 `process_argv`.
- Produces:
  - `git_remote.rs`: `pub struct RemoteRef { pub address: String, pub app: String }`, `pub fn parse_remotes(output: &str) -> Vec<RemoteRef>` (lines of `git remote -v`, scp style `user@host:path` only, app = last path segment minus `.git`), `pub fn read_remotes() -> Vec<RemoteRef>` (runs `git remote -v`; any failure is an empty list).
  - `target.rs`: `pub enum Source { Flag, GitRemote, Default }`, `pub enum Target { Local, Remote { server: Server, source: Source, app_hint: Option<String> } }`, `pub struct Selection<'a> { pub flag: Option<&'a str>, pub local: bool, pub via_ssh: bool }`, `pub fn resolve(sel: Selection, list: &ServerList, remotes: &[RemoteRef]) -> anyhow::Result<Target>`. `app_hint` is set only for `Source::GitRemote`.
  - `forward.rs`: `pub fn strip_target_args(argv: &[String], consumed: &[usize]) -> Vec<String>`, `pub fn ssh_args(server: &Server, args: &[String], tty: bool, app_hint: Option<&str>) -> Vec<String>`, `pub fn needs_tty(args: &[String]) -> bool`, `pub fn run(server: &Server, args: Vec<String>, app_hint: Option<&str>) -> anyhow::Result<i32>`.
  - CLI: global `-s/--server <NAME>` and `--local` (`conflicts_with`), hidden global `--app-hint <APP>`. `pub fn require_app(typed: Option<String>, hint: Option<&str>) -> anyhow::Result<String>` (typed wins, error `app name required` otherwise).

Optional app rule: `app` becomes `Option<String>` only where no required positional follows it: deploy, destroy, logs (process keeps its `*` default), restart, stop, rollback, backup, config show, config live, config app, addon bind, addon unbind. Keep `app: String` on run, restore, config get, config set, config unset, and the hidden git commands, since an omitted app there cannot be told apart from the next argument. The app hint travels as `--app-hint` so the server applies the same `require_app`.

- [ ] **Step 1: Write failing tests**

```rust
// target_tests.rs, list has edge=riku@edge.example (default) and prod=riku@prod.example
#[test] fn flag_beats_git_remote_and_default() { /* flag prod, remote edge -> prod, Source::Flag, no hint */ }
#[test] fn git_remote_beats_default_and_gives_app() { /* remote riku@prod.example:shop -> prod, hint Some("shop") */ }
#[test] fn two_matching_remotes_on_different_servers_is_an_error() { /* err text contains "-s" */ }
#[test] fn unregistered_remote_host_is_ignored() { /* remote riku@10.0.0.9:x -> default edge */ }
#[test] fn ip_does_not_match_registered_hostname() { /* remote riku@192.168.1.20:x, no default -> Local */ }
#[test] fn local_flag_forces_local() { /* local: true -> Local */ }
#[test] fn ssh_invocation_never_forwards() { /* via_ssh: true with default set -> Local */ }
#[test] fn unknown_flag_server_is_an_error_listing_servers() { /* flag "nope" -> err contains "edge" and "prod" */ }

// git_remote_tests.rs
#[test] fn parses_scp_style_and_home_path_remotes() {
    let out = "riku\triku@host:myapp (push)\nold\triku@host:~/x/shop.git (fetch)\ngh\thttps://github.com/a/b.git (fetch)\n";
    let apps: Vec<_> = parse_remotes(out).into_iter().map(|r| (r.address, r.app)).collect();
    assert_eq!(apps, [("riku@host".into(), "myapp".into()), ("riku@host".into(), "shop".into())]);
}

// forward_tests.rs
#[test] fn strips_only_consumed_server_tokens() {
    let argv = sv(&["riku", "-s", "prod", "run", "myapp", "grep", "-s", "x"]);
    assert_eq!(strip_target_args(&argv, &[1, 2]), sv(&["run", "myapp", "grep", "-s", "x"]));
}
#[test] fn ssh_args_quote_each_argument_and_skip_riku_word() {
    let a = ssh_args(&edge(), &sv(&["run", "myapp", "echo", "a b"]), true, None);
    assert_eq!(a, sv(&["-t", "--", "riku@edge.example", "run myapp echo 'a b'"]));
}
#[test] fn app_hint_added_only_when_present() { /* Some("shop") -> remote string starts with "--app-hint shop " ; None -> no "--app-hint" */ }
#[test] fn tty_only_for_run_and_follow() { /* run -> true, logs -f -> true, logs -> false, ps -> false */ }
#[test] fn client_plugin_receives_selected_server() { /* routing: build_plugin_args(cmd, Some(&addr))[0] == "riku@edge.example" */ }
```

`strip_target_args` drops argv[0] and the consumed indices; callers compute indices from `ArgMatches::indices_of("server")` (and the flag token before a separate value, when argv[i-1] is `-s` or `--server`) and `indices_of("local")`. `ssh_args` joins the remote words with `shlex::try_join`.

- [ ] **Step 2: Run to verify failure**, `cargo test -p riku-cli remote:: routing`

- [ ] **Step 3: Implement**

`main.rs` order: parse; client plugin check (now passing the server address when a remote target resolves); then if placement is Forwarded and target is Remote, `eprintln!("[*] {} ({})", name, address)` and `std::process::exit(forward::run(..))`; if placement is Server (except dashboard) and target is Remote, error `'<cmd>' runs on the server itself; use --local to run it here`; otherwise run locally as today, passing `--app-hint` to `require_app`. Only load `servers.toml` and run `git remote -v` when the command is Forwarded or dashboard and `SSH_ORIGINAL_COMMAND` is unset. The help heading becomes `Forwarded commands (run on <name>, or here with --local):` when the default or git remote resolves (the `-s` flag is not pre-scanned for help), else the Task 4 text.

- [ ] **Step 4: Verify**

Run: `cargo test -q`. Then manual: `riku server add` is Task 7, so seed `servers.toml` by hand, `cargo run -q -- -s edge ps`, and `cargo run -q -- -s edge run myapp echo "a b"` prints `a b` with the stderr target line. Report unverified if edge is unreachable.

- [ ] **Step 5: Commit** `Forward commands to a selected server`

---

### Task 7: `riku server` commands

**Files:**
- Create: `crates/riku-cli/src/remote/server_cmd.rs`
- Modify: `crates/riku-cli/src/cmds.rs` (`ServerCmd`), `crates/riku-cli/src/cli.rs` (`Server(ServerCmd)`), `crates/riku/src/main.rs`
- Test: `tests/integration_tests/cli_tests.rs`

**Interfaces:**
- Consumes: Task 5 `load`, `save`, `servers_path`, `ServerList`.
- Produces: `ServerCmd::{Add { name, address, admin: Option<String> }, List, Remove { name }, Default { name }}`.

- [ ] **Step 1: Write failing integration test** `server_add_list_default_remove_round_trip`: run the binary with `XDG_CONFIG_HOME` set to a temp dir (per process env on the `Command`, not the test process), add two servers, set default, `list` output marks the default with `*`, remove the default, `list` shows none marked. Also `server_add_rejects_bad_address` asserts exit code 2 or 1 and the stderr names the bad value.

- [ ] **Step 2: Run to verify failure**, `cargo test --test integration_tests server_`

- [ ] **Step 3: Implement** handlers: load, mutate, save, print with `display::print_table` (`NAME`, `ADDRESS`, `ADMIN`, default marked `*`).

- [ ] **Step 4: Verify**, `cargo test -q`, PASS.

- [ ] **Step 5: Commit** `Add riku server commands`

---

### Task 8: Remote dashboard session

**Files:**
- Create: `crates/riku-cli/src/remote/dashboard/{mod.rs,ssh_master.rs,remote_script.rs,remote_script_tests.rs,sudo.rs,browser.rs}`
- Modify: `crates/riku-dashboard/src/lib.rs` (report bound port), `crates/riku/src/main.rs` (dashboard arm)

**Interfaces:**
- Consumes: Task 6 `Target`, Task 5 `Server`.
- Produces: `pub fn run_remote(server: &Server) -> anyhow::Result<()>`; in `riku-dashboard`, after binding print exactly `riku-dashboard-bound <port>` on its own stdout line (plain `println!`, no color) using `listener.local_addr()`, and make the human line use the real address too.

Pinned constants: `PREFERRED_LOCAL_PORT: u16 = 8088`, `BOUND_PREFIX: &str = "riku-dashboard-bound "`, `TOKEN_BYTES: usize = 32` (hex encoded), control path `<tempdir>/%C`.

Remote script (exact, built by `remote_script.rs`, `<deploy>` is `server.address.user()` passed as an argument, never interpolated):

```sh
sudo <SUDO_FLAGS> -H -u "$1" sh -c '
  IFS= read -r t
  export RIKU_DASHBOARD_TOKEN="$t"
  bin=$(command -v riku || echo "$HOME/.local/bin/riku")
  "$bin" dashboard --bind 127.0.0.1:0 &
  p=$!
  cat >/dev/null
  kill "$p"
'
```

`<SUDO_FLAGS>` is `-n` or `-S -p ''`. Stdin payload is `token\n` or `password\ntoken\n`; stdin stays open for the session, so closing it (Ctrl-C, dropped link) ends `cat` and kills the dashboard.

Session steps in `mod.rs`: require `server.admin` (error: `no admin login for <name>, add one: riku server add <name> <address> --admin user@host`); create tempdir; start master `ssh -M -S <ctl> -o ControlPersist=yes -N <admin>` and poll `ssh -S <ctl> -O check <admin>` until ok (10 s limit); probe `sudo -n -u <deploy> true`, and if stderr says a password is required, prompt with echo disabled via `nix::sys::termios` (restore the terminal on every path); spawn the script; read stdout until `BOUND_PREFIX`, forwarding other lines to stderr; pick local port (bind test 8088, else port 0); `ssh -S <ctl> -O forward -L <local>:127.0.0.1:<remote> <admin>`; open `http://127.0.0.1:<local>/?token=<token>` and print it; wait for Ctrl-C or child exit; then drop stdin, wait for the child, `ssh -S <ctl> -O exit <admin>`. Zero the password buffer after writing it.

- [ ] **Step 1: Write failing tests** (`remote_script_tests.rs`)

```rust
#[test] fn payload_without_password_is_token_line() { assert_eq!(payload(None, "abc"), "abc\n"); }
#[test] fn payload_with_password_puts_it_first() { assert_eq!(payload(Some("pw"), "abc"), "pw\nabc\n"); }
#[test] fn token_never_appears_in_ssh_arguments() { /* build the full ssh argv for a session; assert no element contains the token */ }
#[test] fn deploy_user_is_passed_as_argument_not_spliced() { /* script text contains "$1" and not "riku" user literal; argv ends with the user */ }
#[test] fn parses_bound_port_line() { assert_eq!(parse_bound("riku-dashboard-bound 43121"), Some(43121)); assert_eq!(parse_bound("-----> Dashboard listening"), None); }
#[test] fn control_path_uses_ssh_hash_token() { /* ends with "/%C" */ }
```

Plus in `riku-dashboard`: `reports_real_port_when_binding_zero` binds `127.0.0.1:0` through the function that formats the report line and asserts the port is nonzero.

- [ ] **Step 2: Run to verify failure**, `cargo test -p riku-cli dashboard && cargo test -p riku-dashboard`

- [ ] **Step 3: Implement** per Interfaces and steps above. The sudo password prompt and the ssh master are not unit testable; say so in a comment at the top of `sudo.rs` and `ssh_master.rs` (rule 19).

- [ ] **Step 4: Verify on a real host**

Run from the laptop: `cargo run -q -- server add edge riku@edge.example --admin admin@edge.example`, then `cargo run -q -- dashboard -s edge`. Expected: browser opens, a restart action works, `ps aux | grep RIKU` on the server shows no token, a second terminal running the same command gets its own port, Ctrl-C in one leaves the other running and removes its own remote process. Repeat once with sudo NOPASSWD removed to exercise the password path (this also confirms `sudo -S` reads only the first line, which this plan has not verified). If edge is unavailable, report Task 8 as unverified.

- [ ] **Step 5: Commit** `Open a remote dashboard in one command`

---

### Task 9: Docs

**Files:**
- Modify: `docs/docs/cli.md`, `docs/docs/quick-start.md`, `.claude/CLAUDE.md` (project structure and new module), `API.md` if it lists commands

- [ ] **Step 1:** Document `riku server`, `-s`, `--local`, repo detection, the dashboard command, and the `doctor --fix` migration with the "install the binary first" warning.
- [ ] **Step 2:** Run `mkdocs build --strict` in `docs/` if mkdocs is installed. Expected: no warnings.
- [ ] **Step 3:** Run the full gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -q`, record totals.
- [ ] **Step 4: Commit** `Document remote servers`
