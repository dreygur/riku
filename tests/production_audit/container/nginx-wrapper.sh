#!/bin/sh
# Shadows /usr/sbin/nginx for the riku user. src/nginx/template.rs's
# install_nginx_symlink() calls `Command::new("nginx").args(["-s", "reload"])`
# directly (no sudo wrapping in riku itself) to reload nginx after writing a
# new per-app vhost symlink. The nginx master process runs as root, so a
# non-root riku user cannot signal it via a plain `nginx -s reload` call —
# this wrapper, placed earlier in PATH than /usr/sbin/nginx, transparently
# elevates via a narrowly-scoped passwordless sudo rule (see sudoers-riku-nginx).
exec sudo -n /usr/sbin/nginx "$@"
