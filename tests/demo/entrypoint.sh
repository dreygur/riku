#!/usr/bin/env bash
# entrypoint.sh: container init for the persistent riku demo environment.
#
# Same boot sequence as ../stress/container/entrypoint.sh (see that file for
# the detailed rationale on SSH key import order, the forced-command SSH
# setup, and the plugin-bundling reasoning), plus:
#   - the crates/riku-dashboard embedded API server (`riku dashboard`),
#     started on 127.0.0.1:8088, the actual backend the Next.js dashboard
#     talks to (see dashboard/app/api/riku/[...path]/route.ts).
#   - the Next.js dashboard UI, started on 127.0.0.1:3100, reached only
#     through nginx's dashboard.localhost vhost (nginx-site.conf).
#   - five demo apps (tests/demo/apps/*), deployed via a real `git push`
#     against each app's own bare repo: over the loopback filesystem path,
#     not SSH, so this needs no keypair to work. This is the same mechanism
#     a real `git push riku main` uses (the post-receive hook it installs
#     calls `riku git-hook`), just invoked with a local repo path instead of
#     an ssh:// remote.
#
# Unlike the automated test target, this container is meant to stay up: it
# does not tear itself down, and run_demo.sh does not stop it after boot,
# only run_demo.sh (re-run) or stop_demo.sh do that.
set -euo pipefail

BOOTSTRAP_DIR="/home/riku/.ssh-bootstrap"
RIKU_SSH_DIR="/home/riku/.ssh"
LOG_DIR="/var/log/riku-demo"
RIKU_ROOT="/home/riku/.riku"
DASHBOARD_TOKEN="${RIKU_DASHBOARD_TOKEN:-demo}"
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
su - riku -c "RIKU_ROOT=$RIKU_ROOT /usr/local/bin/riku init --no-systemd" \
    > "$LOG_DIR/riku-init.log" 2>&1 || {
        echo "[entrypoint] riku init failed, see $LOG_DIR/riku-init.log";
        cat "$LOG_DIR/riku-init.log";
        exit 1;
    }
cat "$LOG_DIR/riku-init.log"
# riku init copies the running binary to ~/.local/bin/riku, the exact path
# the post-receive hook `riku apps create` writes is hardcoded to look for
# (src/cli/apps/create.rs). Deploying any app below before this point would
# silently no-op: the hook would find no riku binary and skip the deploy.

echo "[entrypoint] starting nginx"
nginx -t
service nginx start

echo "[entrypoint] starting riku supervisor as user riku"
su - riku -c "RIKU_ROOT=$RIKU_ROOT /usr/local/bin/riku supervisor" \
    > "$LOG_DIR/riku-supervisor.log" 2>&1 &
SUPERVISOR_PID=$!

echo "[entrypoint] starting dashboard API server on 127.0.0.1:8088"
su - riku -c "RIKU_ROOT=$RIKU_ROOT /usr/local/bin/riku dashboard --bind 127.0.0.1:8088 --token $DASHBOARD_TOKEN" \
    > "$LOG_DIR/riku-dashboard-api.log" 2>&1 &
DASHBOARD_API_PID=$!

echo "[entrypoint] deploying demo apps"
riku_as() { su - riku -c "RIKU_ROOT=$RIKU_ROOT /usr/local/bin/riku $*"; }

deploy_demo_app() {
    local app="$1" src="$2"
    if [ -d "$RIKU_ROOT/apps/$app" ]; then
        echo "[entrypoint] '$app' already deployed, skipping"
        return
    fi

    echo "[entrypoint] creating app '$app'"
    riku_as "apps create $app" >> "$LOG_DIR/demo-deploy.log" 2>&1

    echo "[entrypoint] pushing '$app'"
    local work
    work="$(mktemp -d)"
    cp -r "$src"/. "$work"/
    chown -R riku:riku "$work"
    su - riku -c "
        set -e
        cd '$work'
        git init -q -b main
        git config user.email 'demo@riku.local'
        git config user.name 'Riku Demo'
        git add -A
        git commit -q -m 'demo app: $app'
        git push -q '$RIKU_ROOT/repos/$app.git' HEAD:main
    " >> "$LOG_DIR/demo-deploy.log" 2>&1
    rm -rf "$work"

    echo "[entrypoint] pointing '$app' at ${app}.localhost"
    riku_as "config set $app NGINX_SERVER_NAME=${app}.localhost" >> "$LOG_DIR/demo-deploy.log" 2>&1
}

for dir in /opt/demo-apps/*/; do
    app="$(basename "$dir")"
    deploy_demo_app "$app" "$dir"
done

if [ -d "$RIKU_ROOT/apps/hello-python" ] && ! riku_as "addon list" 2>/dev/null | grep -q "demodb"; then
    echo "[entrypoint] provisioning sqlite-volume addon 'demodb' and binding it to hello-python"
    riku_as "addon create sqlite-volume demodb" >> "$LOG_DIR/demo-deploy.log" 2>&1
    riku_as "addon bind demodb hello-python" >> "$LOG_DIR/demo-deploy.log" 2>&1
    riku_as "restart hello-python" >> "$LOG_DIR/demo-deploy.log" 2>&1
fi

echo "[entrypoint] starting dashboard UI on 127.0.0.1:3100"
RIKU_API_URL="http://127.0.0.1:8088" \
RIKU_DASHBOARD_TOKEN="$DASHBOARD_TOKEN" \
RIKU_ROOT="$RIKU_ROOT" \
    /opt/dashboard/node_modules/.bin/next start /opt/dashboard -p 3100 -H 127.0.0.1 \
    > "$LOG_DIR/dashboard.log" 2>&1 &
DASHBOARD_UI_PID=$!

echo "[entrypoint] all services started: sshd=$SSHD_PID supervisor=$SUPERVISOR_PID dashboard-api=$DASHBOARD_API_PID dashboard-ui=$DASHBOARD_UI_PID"
echo "[entrypoint] tailing logs in foreground"

touch /var/log/nginx/access.log /var/log/nginx/error.log
tail -F \
    "$LOG_DIR/sshd.log" \
    "$LOG_DIR/riku-supervisor.log" \
    "$LOG_DIR/riku-dashboard-api.log" \
    "$LOG_DIR/dashboard.log" \
    "$LOG_DIR/demo-deploy.log" \
    /var/log/nginx/access.log \
    /var/log/nginx/error.log &
TAIL_PID=$!

term_handler() {
    echo "[entrypoint] caught termination signal, shutting down"
    kill "$SUPERVISOR_PID" "$DASHBOARD_API_PID" "$DASHBOARD_UI_PID" "$SSHD_PID" "$TAIL_PID" 2>/dev/null || true
    service nginx stop || true
    wait
    exit 0
}
trap term_handler SIGTERM SIGINT

wait "$SSHD_PID"
