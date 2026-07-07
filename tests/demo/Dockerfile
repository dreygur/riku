# Persistent local demo environment: a single container running the real
# riku supervisor, the real dashboard (crates/riku-dashboard's embedded API,
# fronted by the Next.js UI in dashboard/), real nginx, and sshd — with 5
# demo apps (one per bundled runtime plugin, plus one multi-process app)
# deployed automatically at boot via the real git-push/post-receive path, so
# you can open the dashboard and see a populated, working riku instance
# immediately after `docker compose up --build`.
#
# This is NOT the automated test target (see ../stress/container/ for that)
# — this one stays running after boot so a human can poke at it. It reuses
# that target's sshd_config, sudoers/nginx-wrapper plumbing rather than
# duplicating them.
FROM ubuntu:24.04
# See ../stress/container/Dockerfile for why 24.04: matches the glibc
# version of whatever host built the riku binary being COPYed in.

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    openssh-server \
    nginx \
    git \
    curl \
    ca-certificates \
    gnupg \
    python3 \
    python3-pip \
    python3-venv \
    ruby \
    golang-go \
    build-essential \
    sudo \
    procps \
    && rm -rf /var/lib/apt/lists/*

# Node 20.x via NodeSource: Next.js 16 / React 19 need a newer Node than
# Ubuntu 24.04's apt-provided 18.x ships, and only the dashboard (built and
# run as part of this image) needs Node at all — nothing riku-side does.
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# nub (https://nubjs.com): the package manager dashboard/ is pinned to
# (packageManager field in dashboard/package.json, lockfile is lock.yaml).
RUN npm install -g @nubjs/nub

# Dedicated, non-root deploy user — same model as the automated test target.
RUN useradd -m -s /bin/bash riku \
    && usermod -p '*' riku \
    && mkdir -p /home/riku/.ssh \
    && chmod 700 /home/riku/.ssh \
    && chown -R riku:riku /home/riku/.ssh

# `riku apps create` runs `git init --bare` with no --initial-branch, so a
# fresh bare repo's HEAD follows git's own init.defaultBranch. Every demo app
# pushes to `main`; without this, HEAD points at the nonexistent `refs/heads/
# master` and the post-receive hook's `git clone` of that bare repo (see
# src/cli/git/hook.rs) checks out nothing, deploying an empty tree.
RUN su - riku -c "git config --global init.defaultBranch main"

RUN mkdir -p /var/run/sshd \
    && ssh-keygen -A
COPY tests/stress/container/sshd_config /etc/ssh/sshd_config

ARG RIKU_BINARY=target/release/riku
COPY ${RIKU_BINARY} /usr/local/bin/riku
RUN chmod 755 /usr/local/bin/riku

# Static nginx vhosts: the dashboard's own (fixed) vhost, plus the same
# catch-all + include-riku's-generated-per-app-configs pattern the
# automated test target uses.
COPY tests/demo/nginx-site.conf /etc/nginx/sites-available/riku.conf
RUN rm -f /etc/nginx/sites-enabled/default \
    && ln -sf /etc/nginx/sites-available/riku.conf /etc/nginx/sites-enabled/riku.conf \
    && mkdir -p /home/riku/.riku/nginx \
    && chown -R riku:riku /home/riku/.riku \
    && touch /home/riku/.riku/nginx/.keep

# riku (running as the unprivileged riku user) needs to write per-app
# symlinks into /etc/nginx/sites-enabled/ and reload nginx — see
# ../stress/container/Dockerfile for the full rationale, reused verbatim
# here.
RUN chgrp riku /etc/nginx/sites-enabled \
    && chmod g+w /etc/nginx/sites-enabled
COPY tests/stress/container/sudoers-riku-nginx /etc/sudoers.d/riku-nginx
RUN chmod 440 /etc/sudoers.d/riku-nginx \
    && visudo -cf /etc/sudoers.d/riku-nginx
COPY tests/stress/container/nginx-wrapper.sh /usr/local/bin/nginx
RUN chmod 755 /usr/local/bin/nginx

# Runtime plugins baked into the image (no network egress to
# raw.githubusercontent.com required at boot) — same reasoning as the
# automated test target. sqlite-volume is an addon bundle, not a runtime
# plugin, but lives in the same plugins/ directory (bundles are discovered
# by riku-plugin.toml presence, not by name).
COPY plugins/ /home/riku/.riku/plugins/
COPY examples/plugins/sqlite-volume/ /home/riku/.riku/plugins/sqlite-volume/
RUN chmod 755 /home/riku/.riku/plugins/* \
    && chmod 755 /home/riku/.riku/plugins/sqlite-volume/bin/addon \
    && chown -R riku:riku /home/riku/.riku/plugins

# Demo apps, deployed automatically by entrypoint.sh — one per bundled
# runtime plugin (node/python/ruby/go), plus one showing off a multi-process
# Procfile (web + worker + cron).
COPY tests/demo/apps/ /opt/demo-apps/

# Dashboard: built at image-build time so container boot doesn't pay the
# `next build` cost every run. package.json + lock.yaml copied (and
# `nub install --frozen-lockfile` run) before the rest of the source so
# re-running run_demo.sh after only an app-code change reuses the cached
# node_modules layer instead of reinstalling every dependency.
COPY dashboard/package.json dashboard/lock.yaml /opt/dashboard/
RUN cd /opt/dashboard && nub install --frozen-lockfile
COPY dashboard/ /opt/dashboard/
RUN cd /opt/dashboard && nub run build

# nub's project lock directory is created on first use (here, by root,
# during the `nub install`/`nub run build` steps above) with mode 755 — not
# world-writable, unlike /tmp itself. Demo apps are built at runtime by the
# non-root `riku` user, whose own nub/npm build would otherwise fail with
# "failed to acquire project lock: Permission denied" the first time it
# tries to create a lock file root doesn't already own. Clearing it lets
# riku's first build recreate it, owned by riku, for every build after.
RUN rm -rf /tmp/fslock

COPY tests/demo/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 755 /usr/local/bin/entrypoint.sh

EXPOSE 2222 80

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
