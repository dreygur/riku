#!/usr/bin/env bash
# Shared helpers for Docker E2E tests.
#
# All tests source this file.  Each test receives an isolated RIKU_ROOT via
# the test-entrypoint runner so no cleanup of the home directory is needed.

# ---------------------------------------------------------------------------
# Directory structure helpers
# ---------------------------------------------------------------------------

# setup_app APP
#   Create the full ~/.riku directory tree for APP and install the
#   post-receive hook that matches what riku generates (see src/cli/git/repo.rs
#   POST_RECEIVE_HOOK constant).
setup_app() {
    local app="$1"

    mkdir -p \
        "$RIKU_ROOT/repos/${app}.git" \
        "$RIKU_ROOT/apps/${app}" \
        "$RIKU_ROOT/envs/${app}" \
        "$RIKU_ROOT/workers-available" \
        "$RIKU_ROOT/workers-enabled" \
        "$RIKU_ROOT/nginx" \
        "$RIKU_ROOT/logs/${app}" \
        "$RIKU_ROOT/plugins" \
        "$RIKU_ROOT/data/${app}"

    # Initialise the bare git repo
    git init --bare "$RIKU_ROOT/repos/${app}.git" --quiet

    # Install the post-receive hook.
    # Content mirrors POST_RECEIVE_HOOK in src/cli/git/repo.rs exactly so that
    # the hook correctly invokes: riku git-hook APP REPO_PATH
    local hook="$RIKU_ROOT/repos/${app}.git/hooks/post-receive"
    cat > "$hook" << 'HOOK'
#!/usr/bin/env bash
set -e; set -o pipefail;
# Find riku binary from PATH or use common locations
RIKU_BIN="${RIKU_BIN:-$(command -v riku)}"
if [ -z "$RIKU_BIN" ]; then
    if [ -x "$HOME/.local/bin/riku" ]; then
        RIKU_BIN="$HOME/.local/bin/riku"
    elif [ -x "$HOME/riku" ]; then
        RIKU_BIN="$HOME/riku"
    elif [ -x "/usr/local/bin/riku" ]; then
        RIKU_BIN="/usr/local/bin/riku"
    else
        echo "Error: riku binary not found" >&2
        exit 1
    fi
fi
# Derive app name from the repo directory name (strip .git suffix).
APP="$(basename "$(pwd)" .git)"
REPO_PATH="$(pwd)"
cat | RIKU_ROOT="${RIKU_ROOT:-$HOME/.riku}" "$RIKU_BIN" git-hook "$APP" "$REPO_PATH"
HOOK
    chmod +x "$hook"
}

# ---------------------------------------------------------------------------
# Git push helper
# ---------------------------------------------------------------------------

# push_app APP SRC_DIR
#   Initialise a working git repo in SRC_DIR, commit everything, and push
#   to the bare repo we created in setup_app.
push_app() {
    local app="$1"
    local src_dir="$2"

    pushd "$src_dir" > /dev/null
    git init --quiet
    git config user.email "test@test.com"
    git config user.name "Test"
    git add .
    git commit -m "initial" --quiet
    # Pass RIKU_ROOT through to the hook process
    RIKU_ROOT="$RIKU_ROOT" git push "$RIKU_ROOT/repos/${app}.git" HEAD:main --quiet
    popd > /dev/null
}

# ---------------------------------------------------------------------------
# Assertion helpers
# ---------------------------------------------------------------------------

assert_file_exists() {
    local f="$1"
    if [ ! -f "$f" ]; then
        echo "ASSERTION FAILED: expected file '$f' to exist"
        return 1
    fi
}

assert_dir_exists() {
    local d="$1"
    if [ ! -d "$d" ]; then
        echo "ASSERTION FAILED: expected directory '$d' to exist"
        return 1
    fi
}

assert_file_not_exists() {
    local f="$1"
    if [ -f "$f" ]; then
        echo "ASSERTION FAILED: expected file '$f' to NOT exist"
        return 1
    fi
}

assert_file_contains() {
    local f="$1"
    local pattern="$2"
    if ! grep -q "$pattern" "$f"; then
        echo "ASSERTION FAILED: expected '$pattern' in '$f'"
        echo "Actual content:"
        cat "$f"
        return 1
    fi
}

assert_dir_not_exists() {
    local d="$1"
    if [ -d "$d" ]; then
        echo "ASSERTION FAILED: expected directory '$d' to NOT exist"
        return 1
    fi
}
