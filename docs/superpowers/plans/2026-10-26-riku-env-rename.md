# RIKU_ env rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every env var riku defines is `RIKU_` prefixed and router neutral, and an app ENV still using an old name is reported loudly.

**Architecture:** One table of `(old, new)` names in `riku-util` drives both the warning and the docs. The rename itself is mechanical `sed` over `git grep -l`, longest names first so generic patterns never shadow specific ones.

**Tech Stack:** Rust workspace, Tera templates, POSIX shell plugins, BSD sed (macOS) / GNU sed (CI) compatible commands.

**Spec:** `docs/superpowers/specs/2026-10-26-router-neutral-caddy-design.md` (Part 1)

## Global Constraints

- No `?` in new or touched functions; explicit `match`/`return Err` (CLAUDE.md rule 16), `#[allow(clippy::question_mark)]` where clippy objects.
- Tests in sibling `_tests.rs` via `#[path]`.
- No dashes in prose, no emoji, no AI attribution in commits.
- Kept names: `PORT`, `PATH`, `HOME`, `COMPOSE_FILE`, `UWSGI_*`, `RIKU_*` already prefixed, app owned values (`DATABASE_URL`, `SECRET_KEY`, ...).

## Review Focus

- An app ENV with `NGINX_SERVER_NAME=` set: deploy prints the rename hint and routing no longer uses the value.
- An ENV with both old and new key: only the old one is reported, new value wins.
- A key that merely starts with an old name (`NGINX_SERVER_NAME_EXTRA`): not reported.
- Shell plugins in `plugins/` read the new names (grep proves no old name remains).
- `RIKU_NGINX_SITES_ENABLED` is not renamed to `RIKU_RIKU_NGINX_SITES_ENABLED`.

---

### Task 1: Legacy key table and warning

**Files:**
- Create: `crates/riku-util/src/legacy_env.rs`, `crates/riku-util/src/legacy_env_tests.rs`
- Modify: `crates/riku-util/src/lib.rs` (add `pub mod legacy_env;`)
- Modify: `crates/riku-deploy/src/lib.rs` (after the app env is loaded, before build)
- Modify: `crates/riku-cli/src/doctor/checks.rs` (+ register in `doctor/mod.rs`)

**Interfaces:**
- Produces: `pub const RENAMED_KEYS: &[(&str, &str)]` (old, new), and
  `pub fn legacy_keys(env: &HashMap<String, String>) -> Vec<(&'static str, &'static str)>`
  returning the pairs whose old name is a key in `env`, in table order.

Table contents, exactly (exact names only, no prefix matching):

```
NGINX_SERVER_NAME -> RIKU_DOMAINS
NGINX_HTTPS_ONLY -> RIKU_HTTPS_ONLY
NGINX_INTERNAL_PORT -> RIKU_UPSTREAM_PORT
BIND_ADDRESS -> RIKU_BIND_ADDRESS
DISABLE_IPV6 -> RIKU_DISABLE_IPV6
ACME_EMAIL -> RIKU_ACME_EMAIL
ACME_ROOT_CA -> RIKU_ACME_ROOT_CA
ACME_ROOT -> RIKU_ACME_ROOT
NODE_VERSION -> RIKU_NODE_VERSION
PYTHON_VERSION -> RIKU_PYTHON_VERSION
PYTHON_PACKAGE_MANAGER -> RIKU_PYTHON_PACKAGE_MANAGER
RUNTIME -> RIKU_RUNTIME
GHCR_USERNAME -> RIKU_GHCR_USERNAME
GHCR_TOKEN -> RIKU_GHCR_TOKEN
NGINX_ALLOW_GIT_FOLDERS, NGINX_CACHE_ANY, NGINX_CACHE_CONTROL,
NGINX_CACHE_EXPIRY, NGINX_CACHE_PATH, NGINX_CACHE_PREFIXES,
NGINX_CACHE_REDIRECTS, NGINX_CACHE_SIZE, NGINX_CACHE_TIME, NGINX_CATCH_ALL,
NGINX_CLOUDFLARE_ACL, NGINX_CSP, NGINX_DOCUMENT_ROOT, NGINX_EXTERNAL_PORT,
NGINX_INCLUDE_FILE, NGINX_IPV4_ADDRESS, NGINX_IPV6_ADDRESS, NGINX_STATIC,
NGINX_STATIC_PATHS -> same name with RIKU_ prefix
```

(`NGINX_WSGI`, `NGINX_PORTMAP`, `NGINX_SOCKET`, `NGINX_INCLUDE_CONTENT`,
`NGINX_ROOT` are set by riku itself, never by a user; they are renamed in
Task 2 but are not in this table.)

- [ ] **Step 1: Write the failing tests** in `legacy_env_tests.rs`:
  - `reports_old_keys_with_their_new_name`: env `{NGINX_SERVER_NAME, PORT}` gives `[("NGINX_SERVER_NAME", "RIKU_DOMAINS")]`.
  - `ignores_new_and_lookalike_keys`: env `{RIKU_DOMAINS, NGINX_SERVER_NAME_EXTRA}` gives `[]`.
  - `table_has_no_duplicate_or_unprefixed_targets`: every new name starts with `RIKU_`, no old name repeats.
- [ ] **Step 2:** `cargo test -p riku-util legacy_env` fails (module missing).
- [ ] **Step 3:** Implement the module. Deploy: for each pair, `echo` in yellow `-----> ENV key OLD is no longer read, rename it to NEW`. Doctor: one `Check::warn("app env", "<app>: OLD is no longer read, use NEW")` per app and pair, reading each app's ENV under `paths.env_root`; `Check::ok` when none.
- [ ] **Step 4:** `cargo test -p riku-util legacy_env` and `cargo test -p riku-cli doctor` pass.
- [ ] **Step 5:** Commit `Warn on env keys renamed to RIKU_`.

### Task 2: Mechanical rename

**Files:** everything `git grep -l` finds outside `target/`, lockfiles,
`docs/superpowers/`, and `crates/riku-util/src/legacy_env*.rs` (those must keep
the old names).

- [ ] **Step 1: Rename, specific names first.** For each pair below run
  `git grep -lw OLD -- . ':!docs/superpowers' ':!crates/riku-util/src/legacy_env*' | xargs sed -i '' 's/[[:<:]]OLD[[:>:]]/NEW/g'`
  in this order:
  1. the five neutral pairs from Task 1 (`NGINX_SERVER_NAME` ... `DISABLE_IPV6`)
  2. `ACME_ROOT_CA`, `ACME_EMAIL`, then `ACME_ROOT`
  3. `PYTHON_PACKAGE_MANAGER`, `PYTHON_VERSION`, `NODE_VERSION`, `GHCR_USERNAME`, `GHCR_TOKEN`
  4. `RUNTIME` only where it is the env key: the quoted `"RUNTIME"` in `crates/` and `RUNTIME=` in docs and shell; review each hit, the bare word appears in prose
  5. every remaining `NGINX_[A-Z0-9_]*` env name, including the riku set ones listed in Task 1, to `RIKU_NGINX_...`. Word boundaries keep `RIKU_NGINX_SITES_ENABLED` and `DEFAULT_NGINX_SITES_ENABLED` untouched; Rust constants such as `NGINX_CACHE_SIZE_DEFAULT` may be renamed, that is fine.
- [ ] **Step 2: Prove nothing old is left.**
  `git grep -nwE 'NGINX_[A-Z0-9_]+|BIND_ADDRESS|DISABLE_IPV6|ACME_(EMAIL|ROOT|ROOT_CA)|NODE_VERSION|PYTHON_VERSION|PYTHON_PACKAGE_MANAGER|GHCR_(USERNAME|TOKEN)' -- . ':!docs/superpowers' ':!crates/riku-util/src/legacy_env*' ':!*.lock'`
  Expected: only `RIKU_`-prefixed matches and `DEFAULT_NGINX_SITES_ENABLED`.
- [ ] **Step 3:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast` all pass. Render check: rerun the nginx template tests; a template still naming an old key would render an empty value, so also `git grep -n '{{ *NGINX_' templates` must return nothing.
- [ ] **Step 4:** `git diff --stat` review: no hits in lockfiles, no `RIKU_RIKU_`. `git grep -n RIKU_RIKU_` returns nothing.
- [ ] **Step 5:** Add a "Renamed in 0.1.2" table to `docs/docs/env.md` listing the Task 1 pairs. Build docs with `mkdocs build --strict`.
- [ ] **Step 6:** Commit `Prefix every riku env var with RIKU_`, body naming the kept exceptions.
