#!/usr/bin/env bash
# entrypoint.sh: container init for the persistent riku demo environment.
#
# Boot sequence:
#   - imports any bootstrap SSH public keys and starts sshd.
#   - runs the root `riku init --no-systemd`, the same init a server gets
#     from `sudo riku init`, minus the systemd units this container cannot
#     run. It sets up the deploy user's tree, /run/riku-sockets,
#     /var/lib/riku, /etc/riku/nginx and /etc/nginx/conf.d/riku.conf.
#   - starts what those units would: the router helper as root (it writes
#     app sites to /etc/riku/nginx and reloads nginx), nginx, and the
#     supervisor as the deploy user.
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
# This container is meant to stay up: it does not tear itself down, and
# run_demo.sh does not stop it after boot, only run_demo.sh (re-run) or
# stop_demo.sh do that.
set -euo pipefail

BOOTSTRAP_DIR="/home/riku/.ssh-bootstrap"
BOOTSTRAP_KEYS="/root/bootstrap_keys"
LOG_DIR="/var/log/riku-demo"
RIKU_ROOT="/home/riku/.riku"
DASHBOARD_TOKEN="${RIKU_DASHBOARD_TOKEN:-demo}"
mkdir -p "$LOG_DIR"

# Bootstrap public keys are gathered into one root owned file and handed to
# `riku init --ssh-key`, which authorizes them with riku's forced command.
init_args=(--no-systemd)
: > "$BOOTSTRAP_KEYS"
if [ -d "$BOOTSTRAP_DIR" ]; then
    shopt -s nullglob
    for pub in "$BOOTSTRAP_DIR"/*.pub; do
        cat "$pub" >> "$BOOTSTRAP_KEYS"
        echo "[entrypoint] found bootstrap key $(basename "$pub")"
    done
    shopt -u nullglob
fi
if [ -s "$BOOTSTRAP_KEYS" ]; then
    init_args+=(--ssh-key "$BOOTSTRAP_KEYS")
fi

echo "[entrypoint] starting sshd on port 2222"
/usr/sbin/sshd -D -e >"$LOG_DIR/sshd.log" 2>&1 &
SSHD_PID=$!

echo "[entrypoint] running 'riku init ${init_args[*]}' as root for deploy user riku"
RIKU_USER=riku RIKU_ROUTER=nginx /usr/local/bin/riku init "${init_args[@]}" \
    > "$LOG_DIR/riku-init.log" 2>&1 || {
        echo "[entrypoint] riku init failed, see $LOG_DIR/riku-init.log";
        cat "$LOG_DIR/riku-init.log";
        exit 1;
    }
cat "$LOG_DIR/riku-init.log"

echo "[entrypoint] starting nginx"
nginx -t
service nginx start

# Root init with --no-systemd installs no riku-router-helper.service, so the
# helper runs here, as root, the way that unit would run it. Deploys reach it
# through /run/riku-router.sock and fail without it.
echo "[entrypoint] starting the router helper as root"
RIKU_USER=riku /usr/local/bin/riku router-helper > "$LOG_DIR/riku-router-helper.log" 2>&1 &
ROUTER_HELPER_PID=$!
for _ in $(seq 1 50); do
    [ -S /run/riku-router.sock ] && break
    sleep 0.1
done
if [ ! -S /run/riku-router.sock ]; then
    echo "[entrypoint] router helper did not start, see $LOG_DIR/riku-router-helper.log"
    cat "$LOG_DIR/riku-router-helper.log"
    exit 1
fi

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
    riku_as "settings set $app routing.domains=${app}.localhost" >> "$LOG_DIR/demo-deploy.log" 2>&1
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

echo "[entrypoint] all services started: sshd=$SSHD_PID router-helper=$ROUTER_HELPER_PID supervisor=$SUPERVISOR_PID dashboard-api=$DASHBOARD_API_PID dashboard-ui=$DASHBOARD_UI_PID"
echo "[entrypoint] tailing logs in foreground"

touch /var/log/nginx/access.log /var/log/nginx/error.log
tail -F \
    "$LOG_DIR/sshd.log" \
    "$LOG_DIR/riku-router-helper.log" \
    "$LOG_DIR/riku-supervisor.log" \
    "$LOG_DIR/riku-dashboard-api.log" \
    "$LOG_DIR/dashboard.log" \
    "$LOG_DIR/demo-deploy.log" \
    /var/log/nginx/access.log \
    /var/log/nginx/error.log &
TAIL_PID=$!

term_handler() {
    echo "[entrypoint] caught termination signal, shutting down"
    kill "$SUPERVISOR_PID" "$ROUTER_HELPER_PID" "$DASHBOARD_API_PID" "$DASHBOARD_UI_PID" "$SSHD_PID" "$TAIL_PID" 2>/dev/null || true
    service nginx stop || true
    wait
    exit 0
}
trap term_handler SIGTERM SIGINT

wait "$SSHD_PID"
