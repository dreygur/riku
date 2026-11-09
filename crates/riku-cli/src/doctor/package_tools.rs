//! `riku doctor` recommendation for the package tools the bundled runtime
//! plugins build with: nub for Node apps and uv for Python apps. Without them
//! the plugins fall back to npm and pip, so a missing tool is a warning, not
//! a failure.
//!
//! The python plugin also looks in `~/.local/bin` and `~/.cargo/bin`, where
//! uv's installer puts it, so this check does too.

use std::path::Path;

use super::Check;

/// A package tool a bundled runtime plugin prefers.
struct Tool {
    name: &'static str,
    builds: &'static str,
    fallback: &'static str,
    install: &'static str,
}

const TOOLS: [Tool; 2] = [
    Tool {
        name: "nub",
        builds: "Node",
        fallback: "npm",
        install: "npm install -g @nubjs/nub",
    },
    Tool {
        name: "uv",
        builds: "Python",
        fallback: "pip",
        install: "curl -LsSf https://astral.sh/uv/install.sh | sh",
    },
];

const HOME_BIN_DIRS: [&str; 2] = [".local/bin", ".cargo/bin"];

fn tool_check(tool: &Tool, found: bool) -> Check {
    if found {
        return Check::ok(
            tool.name,
            format!("found: {} apps build with it", tool.builds),
        );
    }
    Check::warn(
        tool.name,
        format!(
            "not found: {} apps build with {} instead; install it with: {}",
            tool.builds, tool.fallback, tool.install
        ),
    )
}

fn installed(name: &str, home: Option<&Path>) -> bool {
    if which::which(name).is_ok() {
        return true;
    }
    match home {
        Some(home) => HOME_BIN_DIRS
            .iter()
            .any(|dir| home.join(dir).join(name).is_file()),
        None => false,
    }
}

/// One check per recommended package tool.
pub fn package_tools(home: Option<&Path>) -> Vec<Check> {
    TOOLS
        .iter()
        .map(|tool| tool_check(tool, installed(tool.name, home)))
        .collect()
}

#[cfg(test)]
#[path = "package_tools_tests.rs"]
mod tests;
