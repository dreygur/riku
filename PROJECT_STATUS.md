# Riku — Project Status

## Current Version: 3.0.0

---

## Plugin-Based Runtime System (completed 2026-04-09)

### What Changed

All hardcoded runtime logic (~3,500 lines across 16 files in `src/deploy/`) has been
removed from the core binary. Runtime handling is now fully delegated to external
plugins in `~/.riku/plugins/`.

The core binary now owns only:

- Git receive and repo sync
- Procfile parsing
- Process supervision
- Nginx configuration generation
- Plugin lifecycle orchestration

### Plugin API Contract

Every runtime plugin is an executable (any language) placed in `~/.riku/plugins/`.
It must implement four subcommands, called with context via environment variables:

| Subcommand | Behaviour |
|---|---|
| `detect` | exit 0 = handles this app, exit 1 = skip |
| `build` | install dependencies; stdout/stderr streamed to deploy log |
| `env` | print `KEY=VALUE` lines to stdout (merged into worker env) |
| `start` | print the default start command (fallback if Procfile has no entry) |

Environment variables passed to all subcommands:

```
RIKU_APP          app name
RIKU_APP_PATH     path to checked-out source code
RIKU_ENV_PATH     path to app's env directory (~/.riku/envs/<app>)
RIKU_ROOT         riku root (~/.riku)
```

**Naming convention:** Runtime plugins are any executable in `~/.riku/plugins/` that
does **not** start with `riku-`. Lifecycle hook plugins keep the `riku-` prefix
(`riku-pre-deploy`, `riku-post-build`, etc.). Both live in the same directory.

**Detection resolution:**

1. If `RUNTIME=<name>` is in the app ENV → skip detection, use that plugin directly
2. Otherwise → run `detect` on all non-`riku-*` plugins sorted alphabetically; first exit 0 wins
3. If no plugin matches → **fail** with a clear error message

### Files Deleted (16 runtime files, ~3,500 lines)

| File | Reason |
|------|--------|
| `src/deploy/python.rs` | moved to `plugins/python` shell script |
| `src/deploy/python_workers.rs` | same |
| `src/deploy/node.rs` | moved to `plugins/node` shell script |
| `src/deploy/node_workers.rs` | same |
| `src/deploy/ruby.rs` | moved to `plugins/ruby` shell script |
| `src/deploy/go.rs` | moved to `plugins/go` shell script |
| `src/deploy/rust.rs` | moved to `plugins/rust-lang` shell script |
| `src/deploy/java.rs` | moved to `crates/riku-plugin-java` |
| `src/deploy/clojure.rs` | moved to `crates/riku-plugin-clojure` |
| `src/deploy/container.rs` | moved to `crates/riku-plugin-container` |
| `src/deploy/container_runtime.rs` | kept (used by `riku container` CLI) |
| `src/deploy/container_workers.rs` | moved to `crates/riku-plugin-container` |
| `src/deploy/container_export.rs` | same |
| `src/deploy/identity.rs` | replaced by "no match → error" |
| `src/deploy/runtime.rs` | replaced by `src/plugins/runtime.rs` |
| `src/deploy/macros.rs` | only used by deleted runtime files |

### Files Added

| File/Directory | Description |
|---|---|
| `src/plugins/runtime.rs` | discover, detect, build, get_env, get_start_cmd |
| `plugins/node` | shell script plugin for Node.js |
| `plugins/python` | shell script plugin for Python |
| `plugins/ruby` | shell script plugin for Ruby |
| `plugins/go` | shell script plugin for Go |
| `plugins/rust-lang` | shell script plugin for Rust |
| `crates/riku-plugin-java/` | Rust binary plugin for Java (Maven/Gradle) |
| `crates/riku-plugin-clojure/` | Rust binary plugin for Clojure (Lein/deps.edn) |
| `crates/riku-plugin-container/` | Rust binary plugin for containers (Docker/Podman) |
| `src/cli/apps/install_plugins.rs` | `riku install-plugins` command |

### Key Implementation Details

**`src/plugins/runtime.rs`** — core plugin dispatch:

```rust
pub struct RuntimePlugin { pub name: String, pub path: PathBuf }

pub struct RuntimeContext<'a> {
    pub app: &'a str,
    pub app_path: &'a Path,
    pub env_path: &'a Path,
    pub riku_root: &'a Path,
    pub app_env: &'a HashMap<String, String>,
}

pub fn discover(plugin_root: &Path) -> Vec<RuntimePlugin>
pub fn detect(plugins: &[RuntimePlugin], app_path: &Path, app_env: &HashMap<String,String>) -> Result<Option<RuntimePlugin>>
pub fn build(plugin: &RuntimePlugin, ctx: &RuntimeContext<'_>) -> Result<()>
pub fn get_env(plugin: &RuntimePlugin, ctx: &RuntimeContext<'_>) -> Result<HashMap<String, String>>
pub fn get_start_cmd(plugin: &RuntimePlugin, ctx: &RuntimeContext<'_>) -> Result<Option<String>>
```

**Deploy flow (`src/deploy/mod.rs`):**

```
1. Sync repo → parse Procfile → load ENV
2. discover() — scan ~/.riku/plugins/ for non-riku-* executables, sorted
3. detect() — RUNTIME= override or first exit-0 plugin alphabetically
4. run_pre_build hook
5. build() — stream stdout/stderr to deploy log
6. get_env() — parse KEY=VALUE output, merge into app env
7. get_start_cmd() — use as fallback if Procfile entry is empty
8. run_post_build hook
9. create_workers_generic() — write TOML configs
10. nginx regeneration → supervisor notification
```

**Workspace layout:**

```
riku/
  Cargo.toml              # [workspace] with explicit member list
  crates/
    riku-plugin-java/
    riku-plugin-clojure/
    riku-plugin-container/
  plugins/                # bundled shell script plugins
  src/                    # main riku binary (root package)
  tests/
```

---

## Installing Bundled Plugins

After building or installing the riku binary:

```bash
# Install all bundled runtime plugins
riku install-plugins

# Install specific plugins only
riku install-plugins --plugins node,python,ruby

# Verify installation
ls ~/.riku/plugins/
```

Shell script plugins (node, python, ruby, go, rust-lang) are downloaded from
`https://raw.githubusercontent.com/dreygur/riku/main/plugins/<name>`.

Rust binary plugins (java, clojure, container) are downloaded from GitHub releases.

---

## Writing a Custom Runtime Plugin

```bash
#!/usr/bin/env bash
set -euo pipefail
CMD="${1:-}"
APP_PATH="${RIKU_APP_PATH:-$(pwd)}"
APP="${RIKU_APP:-app}"

case "$CMD" in
  detect)
    # Exit 0 if this plugin handles the app, 1 otherwise
    [ -f "$APP_PATH/my-marker-file" ] && exit 0
    exit 1
    ;;
  build)
    cd "$APP_PATH"
    # Run build steps here; stdout/stderr go to deploy log
    my-build-tool install
    ;;
  env)
    # Print KEY=VALUE pairs — merged into the worker environment
    echo "MY_RUNTIME_ENV=production"
    ;;
  start)
    # Print the default start command (used if Procfile has no matching entry)
    echo "my-runtime server.conf"
    ;;
  *)
    echo "Unknown subcommand: $CMD" >&2
    exit 1
    ;;
esac
```

Install it:

```bash
cp my-plugin ~/.riku/plugins/my-plugin
chmod +x ~/.riku/plugins/my-plugin
```

Or pin it via ENV so detection is skipped:

```bash
riku config set myapp RUNTIME=my-plugin
```

---

## Test Coverage (as of v3.0.0)

| Suite | Count | Status |
|---|---|---|
| Unit tests (lib) | 263 | passing |
| Integration tests | 191 | passing |
| **Total** | **454** | **all pass** |

Key test additions:
- `src/plugins/runtime.rs` — 10 unit tests for discover/detect/build/env/start
- `tests/integration_tests/e2e_tests.rs` — 7 plugin-detection tests + all full-deploy
  tests updated to use lightweight mock plugins (no npm/pip required on host)

---

## Roadmap

- [ ] `riku install-plugins` binary plugin download from GitHub releases
- [ ] Plugin version pinning (`~/.riku/plugins/.versions`)
- [ ] Plugin update command (`riku update-plugins`)
- [ ] Green/blue deployments
- [ ] Multi-server clustering
