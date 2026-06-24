#!/usr/bin/env bash
# run_container_test.sh — orchestrates the full containerized integration
# test: build host binary, build image, provision SSH key, run container,
# drive traffic, collect logs, produce a verdict.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
RESULT_DIR="$SCRIPT_DIR/../results"
mkdir -p "$RESULT_DIR"
TS="$(date +%Y%m%d_%H%M%S)"
LOG="$RESULT_DIR/run_container_test_${TS}.log"

if command -v docker >/dev/null 2>&1; then
    DOCKER_BIN="docker"
elif command -v podman >/dev/null 2>&1; then
    DOCKER_BIN="podman"
else
    echo "FATAL: neither 'docker' nor 'podman' found on PATH" >&2
    exit 1
fi

IMAGE_NAME="riku-container-audit"
CONTAINER_NAME="riku-container-audit-instance"
SSH_PORT="${RIKU_TEST_SSH_PORT:-2222}"
HTTP_PORT="${RIKU_TEST_HTTP_PORT:-8080}"
APP_NAME="${RIKU_TEST_APP_NAME:-trafficapp}"
KEY_DIR="$(mktemp -d /tmp/riku_container_keys.XXXXXX)"
KEY_PATH="$KEY_DIR/id_ed25519"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

cleanup() {
    log "=== cleanup ==="
    $DOCKER_BIN rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

log "=== run_container_test.sh starting ==="
log "docker_bin=$DOCKER_BIN repo_root=$REPO_ROOT key_dir=$KEY_DIR ssh_port=$SSH_PORT http_port=$HTTP_PORT"

# ---- Step 1: build host binary ----
log "--- step 1: building riku release binary on host ---"
(cd "$REPO_ROOT" && cargo build --release) 2>&1 | tee -a "$LOG"
if [ ! -x "$REPO_ROOT/target/release/riku" ]; then
    log "FATAL: $REPO_ROOT/target/release/riku not found after build"
    exit 1
fi
log "binary ready: $REPO_ROOT/target/release/riku"

# ---- Step 2: build docker image ----
log "--- step 2: building docker image $IMAGE_NAME ---"
$DOCKER_BIN build \
    -f "$SCRIPT_DIR/Dockerfile" \
    --build-arg RIKU_BINARY=target/release/riku \
    -t "$IMAGE_NAME" \
    "$REPO_ROOT" 2>&1 | tee -a "$LOG"

# ---- Step 3: provision SSH keypair for automated git push ----
log "--- step 3: provisioning SSH keypair ---"
ssh-keygen -t ed25519 -f "$KEY_PATH" -N "" -C "riku-container-audit" >>"$LOG" 2>&1
chmod 600 "$KEY_PATH"
log "keypair generated: $KEY_PATH / ${KEY_PATH}.pub"

# ---- Step 4: start container, mount pubkey for entrypoint to import ----
log "--- step 4: starting container ---"
$DOCKER_BIN rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
$DOCKER_BIN run -d \
    --name "$CONTAINER_NAME" \
    -p "${SSH_PORT}:2222" \
    -p "${HTTP_PORT}:80" \
    -v "${KEY_DIR}:/home/riku/.ssh-bootstrap:ro" \
    "$IMAGE_NAME" 2>&1 | tee -a "$LOG"

log "waiting for container services (sshd, riku init, nginx, supervisor) to come up"
SSHD_READY=0
for i in $(seq 1 60); do
    if $DOCKER_BIN exec "$CONTAINER_NAME" sh -c "pgrep -f sshd" >/dev/null 2>&1; then
        SSHD_READY=1
        break
    fi
    sleep 1
done

if [ "$SSHD_READY" -ne 1 ]; then
    log "FATAL: sshd did not start in container within 60s"
    $DOCKER_BIN logs "$CONTAINER_NAME" 2>&1 | tee -a "$LOG"
    exit 1
fi
log "sshd is up; giving riku init + nginx + supervisor a few more seconds"
sleep 5

log "container boot log so far:"
$DOCKER_BIN logs "$CONTAINER_NAME" 2>&1 | tee -a "$LOG"

# ---- Step 5: run the traffic simulation against the container ----
log "--- step 5: running user_traffic_simulation.sh ---"
RIKU_TEST_HOST="localhost" \
RIKU_TEST_SSH_PORT="$SSH_PORT" \
RIKU_TEST_HTTP_PORT="$HTTP_PORT" \
RIKU_TEST_SSH_KEY="$KEY_PATH" \
RIKU_TEST_DURATION="${RIKU_TEST_DURATION:-30}" \
RIKU_TEST_CONCURRENCY="${RIKU_TEST_CONCURRENCY:-80}" \
    "$SCRIPT_DIR/user_traffic_simulation.sh" "$APP_NAME" 2>&1 | tee -a "$LOG"
TRAFFIC_EXIT=${PIPESTATUS[0]}
log "user_traffic_simulation.sh exited with code $TRAFFIC_EXIT"

# ---- Step 6: collect internal container logs ----
log "--- step 6: collecting internal container logs ---"
CONTAINER_LOG_DUMP="$RESULT_DIR/container_internal_logs_${TS}.log"
{
    echo "=== $DOCKER_BIN logs (entrypoint stdout/stderr) ==="
    $DOCKER_BIN logs "$CONTAINER_NAME" 2>&1

    echo
    echo "=== riku supervisor log ==="
    $DOCKER_BIN exec "$CONTAINER_NAME" cat /var/log/riku-container/riku-supervisor.log 2>&1

    echo
    echo "=== riku init log ==="
    $DOCKER_BIN exec "$CONTAINER_NAME" cat /var/log/riku-container/riku-init.log 2>&1

    echo
    echo "=== riku install-plugins log ==="
    $DOCKER_BIN exec "$CONTAINER_NAME" cat /var/log/riku-container/riku-install-plugins.log 2>&1

    echo
    echo "=== nginx error log ==="
    $DOCKER_BIN exec "$CONTAINER_NAME" cat /var/log/nginx/error.log 2>&1

    echo
    echo "=== nginx access log (tail 50) ==="
    $DOCKER_BIN exec "$CONTAINER_NAME" tail -50 /var/log/nginx/access.log 2>&1

    echo
    echo "=== zombie/defunct process check inside container ==="
    $DOCKER_BIN exec "$CONTAINER_NAME" sh -c "ps -eo stat,pid,ppid,cmd | grep -E '^Z' || echo 'no zombies found'" 2>&1

    echo
    echo "=== fd count for riku supervisor process inside container ==="
    $DOCKER_BIN exec "$CONTAINER_NAME" sh -c '
        sup_pid=$(pgrep -f "riku supervisor" | head -1)
        if [ -n "$sup_pid" ]; then
            echo "supervisor_pid=$sup_pid fd_count=$(ls /proc/$sup_pid/fd 2>/dev/null | wc -l)"
        else
            echo "supervisor process not found (may have crashed)"
        fi
    ' 2>&1
} > "$CONTAINER_LOG_DUMP" 2>&1

log "internal logs collected at $CONTAINER_LOG_DUMP"

# ---- Step 7: structured verdict ----
log "--- step 7: verdict ---"
NGINX_502_COUNT=$(grep -c ' 502 ' "$CONTAINER_LOG_DUMP" || true)
NGINX_504_COUNT=$(grep -c ' 504 ' "$CONTAINER_LOG_DUMP" || true)
SUPERVISOR_ALIVE=$($DOCKER_BIN exec "$CONTAINER_NAME" sh -c "pgrep -f 'riku supervisor' >/dev/null && echo yes || echo no" 2>/dev/null)
ZOMBIES_FOUND=$($DOCKER_BIN exec "$CONTAINER_NAME" sh -c "ps -eo stat | grep -c '^Z' || true" 2>/dev/null)

VERDICT_FILE="$RESULT_DIR/verdict_${TS}.txt"
{
    echo "==================================================================="
    echo "RIKU CONTAINER INTEGRATION TEST — VERDICT"
    echo "==================================================================="
    echo "timestamp:               $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "app:                     $APP_NAME"
    echo "ssh_port:                $SSH_PORT"
    echo "http_port:                $HTTP_PORT"
    echo
    echo "deploy_and_traffic_exit:  $TRAFFIC_EXIT  (0 = pass, see user_traffic_simulation log for detail)"
    echo "nginx_502_lines_seen:     $NGINX_502_COUNT"
    echo "nginx_504_lines_seen:     $NGINX_504_COUNT"
    echo "supervisor_alive_at_end:  $SUPERVISOR_ALIVE"
    echo "zombie_processes_found:   $ZOMBIES_FOUND"
    echo
    if [ "$TRAFFIC_EXIT" -eq 0 ] && [ "$NGINX_502_COUNT" -eq 0 ] && [ "$NGINX_504_COUNT" -eq 0 ] \
        && [ "$SUPERVISOR_ALIVE" = "yes" ] && [ "$ZOMBIES_FOUND" -eq 0 ]; then
        echo "OVERALL: PASS — deploy succeeded, no 502/504s, supervisor survived, no zombies."
    else
        echo "OVERALL: FAIL — see component results above for which check failed."
    fi
    echo "==================================================================="
} | tee "$VERDICT_FILE" | tee -a "$LOG"

log "verdict written to $VERDICT_FILE"
log "full log at $LOG"
log "=== run_container_test.sh complete ==="
