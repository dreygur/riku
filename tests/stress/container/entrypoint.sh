#!/usr/bin/env bash
# entrypoint.sh — container init for the Riku integration-test target server.
#
# Order matters and is intentional:
#   1. Import the host-generated test SSH key (mounted read-only at
#      /home/riku/.ssh-bootstrap by run_container_test.sh) into the riku
#      user's ~/.ssh, BEFORE running `riku init`. src/cli/setup/ssh.rs's
#      find_public_keys()/select_key() only auto-adds the key without an
#      interactive prompt when exactly one *.pub file is present — so we
#      must ensure exactly one is there at this point.
#   2. Start sshd on port 2222 (see sshd_config).
#   3. Run `riku init --no-systemd` as the riku user, which creates the
#      ~/.riku directory tree, the global post-receive git hook
#      (src/cli/setup/git_hook.rs), and registers the imported key into
#      ~/.ssh/authorized_keys via setup_authorized_keys()
#      (src/util/ssh_keys.rs) with the forced-command line that routes
#      git pushes to `riku git-receive-pack <app>`.
#   4. Start nginx (serves the static site + Riku's generated per-app
#      configs included from ~/.riku/nginx/*.conf).
#   5. Start `riku supervisor` in the background as the riku user — the
#      custom process supervisor under test (src/supervisor/).
#   6. Tail all relevant logs in the foreground so `docker logs` captures
#      everything for run_container_test.sh's log-collection step.
set -euo pipefail

BOOTSTRAP_DIR="/home/riku/.ssh-bootstrap"
RIKU_SSH_DIR="/home/riku/.ssh"
LOG_DIR="/var/log/riku-container"
mkdir -p "$LOG_DIR"

echo "[entrypoint] importing bootstrap SSH key(s), if any"
if [ -d "$BOOTSTRAP_DIR" ]; then
    shopt -s nullglob
    for pub in "$BOOTSTRAP_DIR"/*.pub; do
        cp "$pub" "$RIKU_SSH_DIR/"
        echo "[entrypoint] imported $(basename "$pub")"
    done
    shopt -u nullglob
fi
chown -R riku:riku "$RIKU_SSH_DIR"
chmod 700 "$RIKU_SSH_DIR"
chmod 600 "$RIKU_SSH_DIR"/*.pub 2>/dev/null || true

PUB_COUNT=$(find "$RIKU_SSH_DIR" -maxdepth 1 -name '*.pub' | wc -l)
echo "[entrypoint] $PUB_COUNT public key(s) present in $RIKU_SSH_DIR"
if [ "$PUB_COUNT" -ne 1 ]; then
    echo "[entrypoint] WARNING: riku init's non-interactive auto-add path" \
         "requires exactly one .pub file (src/cli/setup/ssh.rs select_key());" \
         "found $PUB_COUNT. Init may hang waiting on stdin or skip key setup."
fi

echo "[entrypoint] starting sshd on port 2222"
/usr/sbin/sshd -D -e >"$LOG_DIR/sshd.log" 2>&1 &
SSHD_PID=$!

echo "[entrypoint] running 'riku init --no-systemd' as user riku"
su - riku -c "RIKU_ROOT=/home/riku/.riku /usr/local/bin/riku init --no-systemd" \
    > "$LOG_DIR/riku-init.log" 2>&1 || {
        echo "[entrypoint] riku init failed, see $LOG_DIR/riku-init.log";
        cat "$LOG_DIR/riku-init.log";
        exit 1;
    }
cat "$LOG_DIR/riku-init.log"

echo "[entrypoint] runtime plugins are baked into the image at" \
     "/home/riku/.riku/plugins/ (Dockerfile COPY plugins/) rather than" \
     "fetched here via 'riku install-plugins', which requires HTTPS" \
     "egress to raw.githubusercontent.com that this build environment blocks."

echo "[entrypoint] starting nginx"
nginx -t
service nginx start

echo "[entrypoint] starting riku supervisor as user riku"
su - riku -c "RIKU_ROOT=/home/riku/.riku /usr/local/bin/riku supervisor" \
    > "$LOG_DIR/riku-supervisor.log" 2>&1 &
SUPERVISOR_PID=$!

echo "[entrypoint] all services started: sshd=$SSHD_PID supervisor=$SUPERVISOR_PID"
echo "[entrypoint] tailing logs in foreground"

touch /var/log/nginx/access.log /var/log/nginx/error.log
tail -F \
    "$LOG_DIR/sshd.log" \
    "$LOG_DIR/riku-supervisor.log" \
    /var/log/nginx/access.log \
    /var/log/nginx/error.log &
TAIL_PID=$!

term_handler() {
    echo "[entrypoint] caught termination signal, shutting down"
    kill "$SUPERVISOR_PID" "$SSHD_PID" "$TAIL_PID" 2>/dev/null || true
    service nginx stop || true
    wait
    exit 0
}
trap term_handler SIGTERM SIGINT

wait "$SSHD_PID"
