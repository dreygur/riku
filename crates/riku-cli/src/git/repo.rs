//! The post-receive hook text every repo gets.

use crate::util::ROOT_BINARY;

/// The post-receive hook every repo gets. It runs [`ROOT_BINARY`] and never
/// searches `PATH` or the home directory. The app is the repo directory
/// name without `.git`. It is POSIX `sh`, so it runs where bash is not
/// installed.
///
/// `riku git-hook` reads the "oldrev newrev refname" lines directly from its
/// own stdin, so the hook must not `read` them first: that would starve the
/// child of input and silently deploy nothing on a single-ref push.
pub(crate) fn post_receive_hook() -> String {
    format!(
        r#"#!/bin/sh
APP=$(basename "$(pwd)" .git)
REPO_PATH="$(pwd)"
BINARY="{ROOT_BINARY}"
if [ -x "$BINARY" ]; then
    exec "$BINARY" git-hook "$APP" "$REPO_PATH"
else
    echo "■  riku binary not found at $BINARY" >&2
    exit 1
fi
"#
    )
}

#[cfg(test)]
#[path = "repo_tests.rs"]
mod tests;
