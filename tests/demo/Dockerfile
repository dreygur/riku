# Persistent local demo environment: a single container running the real
# riku supervisor, the real dashboard (crates/riku-dashboard's embedded API,
# fronted by the Next.js UI in dashboard/), real nginx, and sshd, with 5
# demo apps (one per bundled runtime plugin, plus one multi-process app)
# deployed automatically at boot via the real git-push/post-receive path, so
# you can open the dashboard and see a populated, working riku instance
# immediately after `docker compose up --build`.
#
# It stays running after boot so a human can poke at it.
#
# The container has no systemd, so entrypoint.sh runs the root
# `riku init --no-systemd` and then starts by hand what the units would
# start: the router helper (as root), nginx and the supervisor.
FROM ubuntu:24.04
# 24.04 matches the glibc of the host that usually builds the riku binary
# COPYed in below.

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
# run as part of this image) needs Node at all: nothing riku-side does.
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# nub (https://nubjs.com): the package manager dashboard/ is pinned to
# (packageManager field in dashboard/package.json, lockfile is nub.lock).
RUN npm install -g @nubjs/nub

# Dedicated, non-root deploy user, as on a real server.
RUN useradd -m -s /bin/bash riku \
    && usermod -p '*' riku \
    && mkdir -p /home/riku/.ssh \
    && chmod 700 /home/riku/.ssh \
    && chown -R riku:riku /home/riku/.ssh

# `riku init` sets the deploy user's git init.defaultBranch to main, the
# branch every demo app pushes.

RUN mkdir -p /var/run/sshd \
    && ssh-keygen -A
COPY tests/demo/sshd_config /etc/ssh/sshd_config

# Root owned and closed to writers: root init refuses any other binary.
ARG RIKU_BINARY=target/release/riku
COPY ${RIKU_BINARY} /usr/local/bin/riku
RUN chown root:root /usr/local/bin/riku && chmod 755 /usr/local/bin/riku

# The dashboard's own vhost and a catch-all. riku's app sites are not
# included here: root init writes /etc/nginx/conf.d/riku.conf, which loads
# /etc/riku/nginx/*.conf where the router helper writes them.
COPY tests/demo/nginx-site.conf /etc/nginx/sites-available/demo.conf
RUN rm -f /etc/nginx/sites-enabled/default \
    && ln -sf /etc/nginx/sites-available/demo.conf /etc/nginx/sites-enabled/demo.conf \
    && mkdir -p /home/riku/.riku \
    && chown -R riku:riku /home/riku/.riku

# Runtime plugins baked into the image, so boot needs no network egress to
# raw.githubusercontent.com. sqlite-volume is an addon bundle, not a runtime
# plugin, but lives in the same plugins/ directory (bundles are discovered
# by riku-plugin.toml presence, not by name).
COPY plugins/ /home/riku/.riku/plugins/
COPY examples/plugins/sqlite-volume/ /home/riku/.riku/plugins/sqlite-volume/
RUN chmod 755 /home/riku/.riku/plugins/* \
    && chmod 755 /home/riku/.riku/plugins/sqlite-volume/bin/addon \
    && chown -R riku:riku /home/riku/.riku/plugins

# Demo apps, deployed automatically by entrypoint.sh, one per bundled
# runtime plugin (node/python/ruby/go), plus one showing off a multi-process
# Procfile (web + worker + cron).
COPY tests/demo/apps/ /opt/demo-apps/

# Dashboard: built at image-build time so container boot doesn't pay the
# `next build` cost every run. package.json + nub.lock copied (and
# `nub install --frozen-lockfile` run) before the rest of the source so
# re-running run_demo.sh after only an app-code change reuses the cached
# node_modules layer instead of reinstalling every dependency.
COPY dashboard/package.json dashboard/nub.lock /opt/dashboard/
RUN cd /opt/dashboard && nub install --frozen-lockfile
COPY dashboard/ /opt/dashboard/
RUN cd /opt/dashboard && nub run build

# nub's project lock directory is created on first use (here, by root,
# during the `nub install`/`nub run build` steps above) with mode 755, not
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
