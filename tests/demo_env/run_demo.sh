#!/usr/bin/env bash
# run_demo.sh — stands up a persistent, browsable riku demo: one container
# running the real riku supervisor, the real dashboard, real nginx, and
# real sshd, with two example apps deployed behind friendly *.localhost
# Host-based vhosts. Re-running this script is the supported way to test
# again later: it rebuilds the image (cheap once node_modules/cargo deps
# are cached) and replaces the container, so it always reflects whatever's
# currently checked out.
#
# Unlike tests/production_audit/container/run_container_test.sh, this
# script does NOT stop the container when it exits — it's meant to be left
# running so you can open the URLs it prints in your actual browser. Use
# stop_demo.sh when you're done.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if command -v docker >/dev/null 2>&1; then
    DOCKER_BIN="docker"
elif command -v podman >/dev/null 2>&1; then
    DOCKER_BIN="podman"
else
    echo "FATAL: neither 'docker' nor 'podman' found on PATH" >&2
    exit 1
fi

IMAGE_NAME="riku-demo-env"
CONTAINER_NAME="riku-demo-env-instance"
SSH_PORT="${RIKU_DEMO_SSH_PORT:-2222}"
HTTP_PORT="${RIKU_DEMO_HTTP_PORT:-8080}"
KEY_DIR="$SCRIPT_DIR/.keys"
KEY_PATH="$KEY_DIR/id_ed25519"
SSH_OPTS=(-i "$KEY_PATH" -p "$SSH_PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o BatchMode=yes)

# blog and shop both deploy the same stdlib-only fixture app
# (test_web_app), just under different names/DEPLOY_ENV values, purely to
# demonstrate that two independently-deployed apps get independent
# Host-routed *.localhost vhosts side by side with the dashboard.
APPS=(blog shop)

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }

log "=== run_demo.sh starting (docker_bin=$DOCKER_BIN ssh_port=$SSH_PORT http_port=$HTTP_PORT) ==="

# ---- Step 1: build the riku release binary ----
log "--- step 1: building riku release binary ---"
(cd "$REPO_ROOT" && cargo build --release)
if [ ! -x "$REPO_ROOT/target/release/riku" ]; then
    log "FATAL: $REPO_ROOT/target/release/riku not found after build"
    exit 1
fi

# ---- Step 2: build the demo image (dashboard build happens inside) ----
log "--- step 2: building image $IMAGE_NAME ---"
"$DOCKER_BIN" build \
    -f "$SCRIPT_DIR/Dockerfile" \
    --build-arg RIKU_BINARY=target/release/riku \
    -t "$IMAGE_NAME" \
    "$REPO_ROOT"

# ---- Step 3: SSH keypair — generated once, reused on every later re-run ----
mkdir -p "$KEY_DIR"
if [ ! -f "$KEY_PATH" ]; then
    log "--- step 3: generating SSH keypair (first run) ---"
    ssh-keygen -t ed25519 -f "$KEY_PATH" -N "" -C "riku-demo-env" >/dev/null 2>&1
    chmod 600 "$KEY_PATH"
else
    log "--- step 3: reusing existing SSH keypair from $KEY_PATH ---"
fi

# ---- Step 4: replace any previous instance ----
log "--- step 4: starting container $CONTAINER_NAME ---"
"$DOCKER_BIN" rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
"$DOCKER_BIN" run -d \
    --name "$CONTAINER_NAME" \
    -p "127.0.0.1:${SSH_PORT}:2222" \
    -p "127.0.0.1:${HTTP_PORT}:80" \
    -v "${KEY_DIR}:/home/riku/.ssh-bootstrap:ro" \
    "$IMAGE_NAME"

log "waiting for sshd to come up in the container"
SSHD_READY=0
for _ in $(seq 1 60); do
    if "$DOCKER_BIN" exec "$CONTAINER_NAME" sh -c "pgrep -f sshd" >/dev/null 2>&1; then
        SSHD_READY=1
        break
    fi
    sleep 1
done
if [ "$SSHD_READY" -ne 1 ]; then
    log "FATAL: sshd did not start within 60s"
    "$DOCKER_BIN" logs "$CONTAINER_NAME"
    exit 1
fi
log "giving riku init + nginx + supervisor + dashboard a few more seconds to settle"
sleep 6
"$DOCKER_BIN" logs "$CONTAINER_NAME"

# ---- Step 5: deploy the example apps ----
export GIT_SSH_COMMAND="ssh ${SSH_OPTS[*]}"
for app in "${APPS[@]}"; do
    log "--- step 5: deploying example app '$app' ---"
    WORK_DIR="$(mktemp -d "/tmp/riku_demo_${app}.XXXXXX")"
    cp -r "$SCRIPT_DIR/../production_audit/container/test_web_app/." "$WORK_DIR/"
    (
        cd "$WORK_DIR"
        git init -q -b main
        git config user.email "demo@riku.local"
        git config user.name "Riku Demo"
        git add -A
        git commit -q -m "demo app: $app"
        git remote add riku "ssh://riku@127.0.0.1:${SSH_PORT}/${app}" 2>/dev/null \
            || git remote set-url riku "ssh://riku@127.0.0.1:${SSH_PORT}/${app}"
        git push -f riku main
    )
    rm -rf "$WORK_DIR"

    log "pointing '$app' at ${app}.localhost and redeploying with that vhost"
    ssh "${SSH_OPTS[@]}" riku@127.0.0.1 config set "$app" "NGINX_SERVER_NAME=${app}.localhost" "DEPLOY_ENV=${app}-demo"
done

# ---- Step 6: ready ----
cat <<EOF

=====================================================================
riku demo environment is up.

*.localhost addresses resolve to 127.0.0.1 on their own on virtually
every modern OS/browser — no /etc/hosts edit needed. Open:

$(printf '  %-12s http://dashboard.localhost:%s\n' "Dashboard:" "${HTTP_PORT}")
$(for app in "${APPS[@]}"; do printf '  %-12s http://%s.localhost:%s\n' "${app}:" "${app}" "${HTTP_PORT}"; done)

Git remotes for redeploying any app directly:
$(for app in "${APPS[@]}"; do echo "  ssh://riku@127.0.0.1:${SSH_PORT}/${app}"; done)

Re-run this script any time to rebuild + redeploy fresh.
Run stop_demo.sh to tear it down.
=====================================================================
EOF
