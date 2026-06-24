#!/usr/bin/env bash
# entrypoint.sh — container init for the persistent riku demo environment.
#
# Same boot sequence as ../stress/container/entrypoint.sh (see
# that file for the detailed rationale on SSH key import order, the
# forced-command SSH setup, and the plugin-bundling reasoning), plus one
# more service: the Next.js/Hono dashboard, started on 127.0.0.1:3100 and
# reached only through nginx's dashboard.localhost vhost (nginx-site.conf).
#
# Unlike the automated test target, this container is meant to stay up:
# it does not tear itself down, and run_demo.sh does not stop it after
# boot — only run_demo.sh (re-run) or stop_demo.sh do that.
set -euo pipefail

BOOTSTRAP_DIR="/home/riku/.ssh-bootstrap"
RIKU_SSH_DIR="/home/riku/.ssh"
LOG_DIR="/var/log/riku-demo"
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

echo "[entrypoint] starting nginx"
nginx -t
service nginx start

echo "[entrypoint] starting riku supervisor as user riku"
su - riku -c "RIKU_ROOT=/home/riku/.riku /usr/local/bin/riku supervisor" \
    > "$LOG_DIR/riku-supervisor.log" 2>&1 &
SUPERVISOR_PID=$!

echo "[entrypoint] starting dashboard on 127.0.0.1:3100"
RIKU_API_URL="http://127.0.0.1:9091" \
RIKU_ROOT="/home/riku/.riku" \
    /opt/dashboard/node_modules/.bin/next start /opt/dashboard -p 3100 -H 127.0.0.1 \
    > "$LOG_DIR/dashboard.log" 2>&1 &
DASHBOARD_PID=$!

echo "[entrypoint] all services started: sshd=$SSHD_PID supervisor=$SUPERVISOR_PID dashboard=$DASHBOARD_PID"
echo "[entrypoint] tailing logs in foreground"

touch /var/log/nginx/access.log /var/log/nginx/error.log
tail -F \
    "$LOG_DIR/sshd.log" \
    "$LOG_DIR/riku-supervisor.log" \
    "$LOG_DIR/dashboard.log" \
    /var/log/nginx/access.log \
    /var/log/nginx/error.log &
TAIL_PID=$!

term_handler() {
    echo "[entrypoint] caught termination signal, shutting down"
    kill "$SUPERVISOR_PID" "$DASHBOARD_PID" "$SSHD_PID" "$TAIL_PID" 2>/dev/null || true
    service nginx stop || true
    wait
    exit 0
}
trap term_handler SIGTERM SIGINT

wait "$SSHD_PID"
