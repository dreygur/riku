# Containerized target server for Riku integration/traffic testing.
#
# Mirrors the real deploy path verified against source before writing this:
#   - SSH forced-command gating: src/util/ssh_keys.rs::setup_authorized_keys()
#     writes `command="FINGERPRINT=... NAME=default <riku_script> $SSH_ORIGINAL_COMMAND"`.
#     git push therefore runs as: <riku_script> git-receive-pack '<app>'
#     (src/cli/cli.rs Commands::GitReceivePack, src/cli/git/receive_pack.rs).
#   - `riku init --no-systemd` (src/cli/setup/init.rs) auto-adds an SSH key
#     from ~/.ssh/*.pub WITHOUT prompting only if exactly one key is found
#     (src/cli/setup/ssh.rs::find_public_keys / select_key) — this image
#     relies on exactly one pubkey being present at first-boot time.
#   - Web workers get $PORT injected by Riku (src/deploy/workers.rs:212) and
#     are proxied from external port 80 by nginx; the app must bind
#     0.0.0.0:$PORT itself.
#   - Nginx wiring follows contrib/nginx/default.conf's convention: a static
#     site file includes ~/.riku/nginx/*.conf, which Riku generates per app.
FROM ubuntu:24.04
# 24.04 (glibc 2.39) chosen over 22.04 here because the host that builds
# the riku binary (run_container_test.sh step 1, `cargo build --release`)
# may itself ship a newer glibc than 22.04 provides; dynamically linked
# Rust binaries fail at runtime if the container's glibc is older than the
# build host's. If your build host is older, 22.04 works fine too.

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    openssh-server \
    nginx \
    git \
    curl \
    ca-certificates \
    python3 \
    python3-pip \
    python3-venv \
    build-essential \
    sudo \
    procps \
    && rm -rf /var/lib/apt/lists/*

# Dedicated, non-root deploy user. Git pushes and the riku supervisor both
# run as this user, matching the production model (contrib/nginx/default.conf
# references /home/deploy/.riku — we use /home/riku/.riku here instead).
RUN useradd -m -s /bin/bash riku \
    && usermod -p '*' riku \
    && mkdir -p /home/riku/.ssh \
    && chmod 700 /home/riku/.ssh \
    && chown -R riku:riku /home/riku/.ssh
# `useradd` without -p leaves the shadow password field as '!' (locked).
# OpenSSH refuses ALL auth methods, including pubkey, for locked accounts
# regardless of UsePAM (verified: sshd -d logged "User riku not allowed
# because account is locked" even with pubkey-only auth configured below).
# '*' is a deliberately unmatchable hash that unlocks the account for
# pubkey auth without enabling password login.

# SSH server: non-privileged port, key-only auth, no root login.
RUN mkdir -p /var/run/sshd \
    && ssh-keygen -A
COPY tests/production_audit/container/sshd_config /etc/ssh/sshd_config

# Riku binary built on the host (see run_container_test.sh step 1) is bind-
# mounted or copied in at build time from the repo's target/release dir.
ARG RIKU_BINARY=target/release/riku
COPY ${RIKU_BINARY} /usr/local/bin/riku
RUN chmod 755 /usr/local/bin/riku

# Static nginx vhost that includes Riku's generated per-app configs.
COPY tests/production_audit/container/riku-nginx.conf /etc/nginx/sites-available/riku.conf
RUN rm -f /etc/nginx/sites-enabled/default \
    && ln -sf /etc/nginx/sites-available/riku.conf /etc/nginx/sites-enabled/riku.conf \
    && mkdir -p /home/riku/.riku/nginx \
    && chown -R riku:riku /home/riku/.riku \
    && touch /home/riku/.riku/nginx/.keep

# Riku (running as the unprivileged riku user) needs to (a) write a
# per-app symlink into /etc/nginx/sites-enabled/ and (b) signal the
# root-owned nginx master to reload (src/nginx/template.rs
# install_nginx_symlink()). Neither works for a non-root user out of the
# box. Fix (a) with group write access; fix (b) with a narrowly-scoped
# passwordless sudo rule plus a PATH-shadowing wrapper, since riku calls
# the bare `nginx` binary directly with no sudo of its own.
RUN chgrp riku /etc/nginx/sites-enabled \
    && chmod g+w /etc/nginx/sites-enabled
COPY tests/production_audit/container/sudoers-riku-nginx /etc/sudoers.d/riku-nginx
RUN chmod 440 /etc/sudoers.d/riku-nginx \
    && visudo -cf /etc/sudoers.d/riku-nginx
COPY tests/production_audit/container/nginx-wrapper.sh /usr/local/bin/nginx
RUN chmod 755 /usr/local/bin/nginx

# Bundle the repo's runtime plugins directly into the image rather than
# relying on `riku install-plugins`, which fetches each script over HTTPS
# from raw.githubusercontent.com (src/cli/apps/install_plugins.rs) — that
# egress path is blocked in this build environment's network policy even
# though the apt mirror is reachable, so it cannot be used here.
COPY plugins/ /home/riku/.riku/plugins/
RUN chmod 755 /home/riku/.riku/plugins/* \
    && chown -R riku:riku /home/riku/.riku/plugins

COPY tests/production_audit/container/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 755 /usr/local/bin/entrypoint.sh

EXPOSE 2222 80 443

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
