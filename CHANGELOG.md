# Changelog

All notable changes to Riku will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.0.0] - 2026-04-09

### Plugin-Based Runtime System

This release removes all hardcoded runtime logic from the core binary. Runtime
detection and building is now fully delegated to external plugins, making the binary
significantly lighter and allowing any language to be supported without a recompile.

**Binary size impact:** ~3,500 lines of runtime-specific code deleted from the core.
All 263 unit tests and 191 integration tests continue to pass.

### Breaking Changes

- **Runtime plugins must be installed separately.** Run `riku install-plugins` after
  upgrading to download the bundled plugins (node, python, ruby, go, rust-lang, java,
  clojure, container) to `~/.riku/plugins/`. Without plugins, deploys will fail with a
  clear error message.
- **`RUNTIME=<name>` ENV var** now pins which plugin handles an app, replacing the old
  implicit priority system. Example: `riku config set myapp RUNTIME=node`.

### Added

- `src/plugins/runtime.rs` — plugin discovery, detection, build dispatch, env and start command extraction
- `plugins/node` — bundled Node.js shell script plugin (detects `package.json`)
- `plugins/python` — bundled Python shell script plugin (detects `requirements.txt`, `pyproject.toml`)
- `plugins/ruby` — bundled Ruby shell script plugin (detects `Gemfile`)
- `plugins/go` — bundled Go shell script plugin (detects `go.mod`, `Godeps`, `.go` files)
- `plugins/rust-lang` — bundled Rust shell script plugin (detects `Cargo.toml` + `rust-toolchain.toml`)
- `crates/riku-plugin-java` — Rust binary plugin for Java (Maven/Gradle)
- `crates/riku-plugin-clojure` — Rust binary plugin for Clojure (Lein/deps.edn)
- `crates/riku-plugin-container` — Rust binary plugin for containers (Docker/Podman, auto-detected)
- `riku install-plugins` CLI command — downloads bundled plugins from GitHub
- `riku install-plugins --plugins <list>` — install specific plugins only
- Cargo workspace: root package + `crates/riku-plugin-*` sub-crates

### Changed

- `src/deploy/mod.rs` — replaced runtime dispatch with plugin-based orchestration
- `src/deploy/workers.rs` — `create_workers_generic` now accepts `start_cmd: Option<&str>` for plugin-provided fallback command
- `src/plugins/executor.rs` — `plugin_timeout` and `wait_with_timeout` made `pub(crate)` for use by runtime.rs
- Integration tests — all full-deploy tests now use lightweight mock plugins; no npm/pip/bundler required on the test host

### Removed

All 16 hardcoded runtime files from `src/deploy/`:
`python.rs`, `python_workers.rs`, `node.rs`, `node_workers.rs`, `ruby.rs`, `go.rs`,
`rust.rs`, `java.rs`, `clojure.rs`, `container.rs`, `container_workers.rs`,
`container_export.rs`, `identity.rs`, `runtime.rs`, `runtime_tests.rs`, `macros.rs`

---

## [2.2.0] - 2026-02-26

### Production Hardening Refactor

This release closes the remaining gaps between the self-audit findings and a
production-ready state. All 214 tests continue to pass; `cargo clippy -D warnings`
is clean with zero warnings in production code.

### Breaking Changes

- **`PIKU_AUTO_RESTART` renamed to `RIKU_AUTO_RESTART`** — update your `ENV` files.
  The old variable name was a residual from the Python Piku port and has now been
  fully removed. All runtimes (Python, Node, Ruby, Go, Java, Clojure, Rust,
  Container, Identity) and all documentation now use the correct `RIKU_AUTO_RESTART`.

### Security Fixes

- **`cargo audit --deny warnings` now blocks releases** — CI will fail on any known
  CVE in the dependency tree instead of silently reporting it (`ci.yml`).
- **Nginx security headers hardened** — `nginx_static.conf.tera` and
  `nginx_portmap.conf.tera` now include `Referrer-Policy` and `Permissions-Policy`
  headers (the HTTPS-only template already had `HSTS`; `nginx_common.conf.tera`
  already had the full set).
- **Systemd `ReadWritePaths` tilde expansion fixed** — `setup.rs` now writes the
  absolute path to `~/.riku` (resolved at runtime) instead of the literal `~/.riku`
  string, which is not expanded by systemd on all distributions.
- **Predictable `/tmp` test path removed** — `tests/deploy/test-all.sh` now uses
  `mktemp -d` instead of the PID-based `/tmp/riku-test-$$` path that was vulnerable
  to symlink attacks.

### Dependency Upgrades

- **`reqwest` upgraded from v0.11 to v0.12** — v0.11 is in maintenance-only mode;
  v0.12 brings `hyper` 1.x, `http` 1.x, and updated TLS dependencies.

### Code Quality

- **All `unwrap()` calls in production paths eliminated** — replaced with
  `unwrap_or_default()` (for infallible `SystemTime` operations) and
  `ok_or_else(|| anyhow!(...))` (for path operations in `setup.rs` and `apps.rs`).
- **Duplicate `create_identity_workers` removed** — the ~170-line copy in
  `deploy/mod.rs` was dead code shadowing the canonical implementation in
  `deploy/identity.rs`. Only the `identity.rs` version remains.
- **Dead code suppressions removed or resolved**:
  - `#[allow(dead_code)]` removed from `deploy_identity` and `create_identity_workers`
    in `identity.rs` (they were already being called).
  - `#[allow(dead_code)]` removed from `remove_nginx_config` and
    `generate_acme_nginx_config` in `nginx.rs`; both are now wired into callers
    (`cmd_destroy` uses `remove_nginx_config`; `cmd_init` calls
    `generate_acme_nginx_config` for the ACME bootstrap config).
  - `install_systemd_service` (system-wide, root) is now called from `cmd_init`
    when running as root, removing its dead-code status.
  - `install_nginx_default_config` and `num_cpus` (genuinely unused) removed entirely.
- **Clippy clean** — `cargo clippy -- -D warnings` passes with zero errors or
  warnings in production code. Fixed 8 `useless_format!` instances across deploy
  modules and 1 `io_other_error` in `supervisor/stats.rs`.
- **`CONTRIBUTING.md` clone URL corrected** — was pointing to `piku.git`, now
  correctly points to `riku.git`.
- **`Runtime::Identity` variant now constructed** — the `None` branch in `do_deploy`
  now calls `found_app(&Runtime::Identity.to_string())` before dispatching, making
  the variant active and removing the dead-code warning.

### Documentation

- All references to `PIKU_AUTO_RESTART` updated to `RIKU_AUTO_RESTART` in README,
  docs site (env.md, faq.md), examples/README.md, API.md, and test scripts.
- `API.md` reference to `PIKU_RAW_SOURCE_URL` updated to `RIKU_RAW_SOURCE_URL`.

---

## [1.0.0] - 2026-02-23

### 🎉 First Stable Release

Riku 1.0.0 is the first stable release of the Rust port of Piku, providing Heroku-like git push deployments.

### ✨ New Features

#### AI Agent Interface
- SSH-based automation for AI agents (Claude, Cursor, Copilot, etc.)
- Permission scopes: `readonly`, `staging`, `production`
- JSON output mode for reliable AI parsing
- Confirmation flow for destructive operations
- Rate limiting per agent
- Audit logging of all AI actions
- Commands: `agent --intro`, `agent --schema`, `agent <command>`

#### Documentation
- Comprehensive mkdocs documentation site
- CLI reference with all commands
- Environment variables guide
- Runtime-specific deployment guides
- Nginx configuration documentation
- Process supervisor documentation
- Plugin system documentation
- AI Agents integration guide
- Systemd integration guide

#### Developer Experience
- Updated `.gitignore` with comprehensive Rust project ignores
- Fixed GitHub Actions workflow formatting
- Code formatting with `cargo fmt`
- Linting with `cargo clippy`

### 🔧 Improvements

- Fixed repository URLs (piku → riku)
- Improved SSH key scope parsing for AI agents
- Wired real deploy/destroy/restart/stop functions to agent commands
- Enhanced error handling with structured JSON responses
- Added confirmation tokens for destructive operations

### 📦 Technical Changes

- All 77 integration tests passing
- Release build optimized with LTO
- Documentation builds with mkdocs-material theme
- GitHub Actions CI/CD pipeline configured

### 📝 Documentation Updates

- Moved SYSTEMD.md to mkdocs
- Fixed incorrect repository references
- Added AI Agent Interface section to README
- Updated installation instructions

---

## [0.1.0] - 2026-02-17

### Initial Pre-release

Initial Rust port of Piku with core functionality:

- Git push deployments
- Multi-language support (Python, Node.js, Ruby, Go, Java, Clojure, Rust)
- Custom Rust process supervisor
- Nginx configuration generation
- Plugin system
- Cron job support
- Environment variable management
- Scaling support

### Test Coverage

- 109 unit tests
- 77 integration tests
- 11 deployment tests
- Total: 197 tests

---

## Version History

| Version | Date | Description |
|---------|------|-------------|
| 1.0.0 | 2026-02-23 | First stable release with AI Agent Interface |
| 0.1.0 | 2026-02-17 | Initial pre-release |
