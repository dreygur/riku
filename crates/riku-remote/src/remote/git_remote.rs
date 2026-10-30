//! Reads the current repo's git remotes to find a riku server and app.
//!
//! Security model: remote URLs come from the repo's git config, which may
//! belong to a repo the user did not write. They are only ever compared
//! against registered server addresses, so a remote can pick among servers
//! the user already registered but never adds a new ssh target. The app name
//! taken from a remote is passed to the server as one quoted argument and is
//! validated there like a typed app name. Paths with control characters are
//! dropped so the name can always be shell quoted. `git` runs with stdin
//! closed and stderr discarded, and any failure counts as no remotes.

use std::process::{Command, Stdio};

/// One scp style remote, `user@host:path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRef {
    /// The `user@host` part, compared exactly against registered addresses.
    pub address: String,
    /// Last path segment without a trailing `.git`.
    pub app: String,
}

/// Splits `user@host:path` into address and path. A bracketed IPv6 host
/// keeps its colons. URLs with a scheme, and local paths with a `/` before
/// the colon, are not scp style.
#[allow(clippy::question_mark)]
fn split_scp(url: &str) -> Option<(&str, &str)> {
    if url.contains("://") {
        return None;
    }
    let host_start = match url.find('@') {
        Some(at) => at + 1,
        None => return None,
    };
    let host = &url[host_start..];
    let colon = if host.starts_with('[') {
        host.find("]:").map(|i| host_start + i + 1)
    } else {
        host.find(':').map(|i| host_start + i)
    };
    let colon = match colon {
        Some(colon) => colon,
        None => return None,
    };
    let address = &url[..colon];
    if address.contains('/') {
        return None;
    }
    Some((address, &url[colon + 1..]))
}

fn app_of(path: &str) -> Option<&str> {
    let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let app = last.strip_suffix(".git").unwrap_or(last);
    if app.is_empty() || app.chars().any(char::is_control) {
        return None;
    }
    Some(app)
}

#[allow(clippy::question_mark)]
fn parse_line(line: &str) -> Option<RemoteRef> {
    let url = match line.split_whitespace().nth(1) {
        Some(url) => url,
        None => return None,
    };
    let (address, path) = match split_scp(url) {
        Some(parts) => parts,
        None => return None,
    };
    app_of(path).map(|app| RemoteRef {
        address: address.to_string(),
        app: app.to_string(),
    })
}

/// Parses `git remote -v` output, keeping only scp style remotes.
pub fn parse_remotes(output: &str) -> Vec<RemoteRef> {
    output.lines().filter_map(parse_line).collect()
}

/// Runs `git remote -v` in the current directory. Not a repo, no `git`, or
/// any other failure is an empty list.
pub fn read_remotes() -> Vec<RemoteRef> {
    let output = Command::new("git")
        .args(["remote", "-v"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(out) if out.status.success() => parse_remotes(&String::from_utf8_lossy(&out.stdout)),
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "git_remote_tests.rs"]
mod tests;
