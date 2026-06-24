# Persistent local demo environment: a single container running the real
# riku supervisor, the real Next.js/Hono dashboard, real nginx, and sshd for
# git-push deploys — built so you can browse the dashboard and deployed apps
# from your host browser at friendly *.localhost addresses (see run_demo.sh).
#
# This is NOT the automated test target (see ../production_audit/container/
# for that) — this one stays running after boot so a human can poke at it.
# It reuses that target's sshd_config, sudoers/nginx-wrapper plumbing, and
# test_web_app fixture rather than duplicating them.
FROM ubuntu:24.04
# See ../production_audit/container/Dockerfile for why 24.04: matches the
# glibc version of whatever host built the riku binary being COPYed in.

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

# Dedicated, non-root deploy user — same model as the automated test target.
RUN useradd -m -s /bin/bash riku \
    && usermod -p '*' riku \
    && mkdir -p /home/riku/.ssh \
    && chmod 700 /home/riku/.ssh \
    && chown -R riku:riku /home/riku/.ssh

RUN mkdir -p /var/run/sshd \
    && ssh-keygen -A
COPY tests/production_audit/container/sshd_config /etc/ssh/sshd_config

ARG RIKU_BINARY=target/release/riku
COPY ${RIKU_BINARY} /usr/local/bin/riku
RUN chmod 755 /usr/local/bin/riku

# Static nginx vhosts: the dashboard's own (fixed) vhost, plus the same
# catch-all + include-riku's-generated-per-app-configs pattern the
# automated test target uses.
COPY tests/demo_env/nginx-site.conf /etc/nginx/sites-available/riku.conf
RUN rm -f /etc/nginx/sites-enabled/default \
    && ln -sf /etc/nginx/sites-available/riku.conf /etc/nginx/sites-enabled/riku.conf \
    && mkdir -p /home/riku/.riku/nginx \
    && chown -R riku:riku /home/riku/.riku \
    && touch /home/riku/.riku/nginx/.keep

# riku (running as the unprivileged riku user) needs to write per-app
# symlinks into /etc/nginx/sites-enabled/ and reload nginx — see
# ../production_audit/container/Dockerfile for the full rationale, reused
# verbatim here.
RUN chgrp riku /etc/nginx/sites-enabled \
    && chmod g+w /etc/nginx/sites-enabled
COPY tests/production_audit/container/sudoers-riku-nginx /etc/sudoers.d/riku-nginx
RUN chmod 440 /etc/sudoers.d/riku-nginx \
    && visudo -cf /etc/sudoers.d/riku-nginx
COPY tests/production_audit/container/nginx-wrapper.sh /usr/local/bin/nginx
RUN chmod 755 /usr/local/bin/nginx

# Runtime plugins baked into the image (no network egress to
# raw.githubusercontent.com required at boot) — same reasoning as the
# automated test target.
COPY plugins/ /home/riku/.riku/plugins/
RUN chmod 755 /home/riku/.riku/plugins/* \
    && chown -R riku:riku /home/riku/.riku/plugins

# Demo app fixture, deployed automatically by entrypoint.sh.
COPY tests/production_audit/container/test_web_app/ /opt/demo-apps/test_web_app/

# Dashboard: built at image-build time so container boot doesn't pay the
# `next build` cost every run. package*.json copied (and `npm ci` run)
# before the rest of the source so re-running run_demo.sh after only an
# app-code change reuses the cached node_modules layer instead of
# reinstalling every dependency.
COPY dashboard/package.json dashboard/package-lock.json* /opt/dashboard/
RUN cd /opt/dashboard && npm ci
COPY dashboard/ /opt/dashboard/
RUN cd /opt/dashboard && npm run build

COPY tests/demo_env/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 755 /usr/local/bin/entrypoint.sh

EXPOSE 2222 80

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
