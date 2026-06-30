---
hide:
  - navigation
  - toc
---

<div class="riku-hero" markdown>

<p class="riku-eyebrow">single binary <span class="dot">·</span> no docker <span class="dot">·</span> written in rust</p>

# Deploy with <span class="grad">git push</span>.<br>On a box you own.

<p class="tagline">Riku is the smallest PaaS you've ever seen — one Rust binary that turns a small server into a Heroku-style deploy target. No runtime, no daemons you didn't ask for.</p>

<div class="riku-cta" markdown>
[Get started](quick-start.md){ .md-button .md-button--primary }
[Install](installation.md){ .md-button }
[GitHub](https://github.com/dreygur/riku){ .md-button }
</div>

<div class="riku-term">
  <div class="riku-term__bar"><i></i><i></i><i></i><span>deploy@your-server</span></div>
<pre><code><span class="l"><span class="p">$</span> <span class="cmd">git push riku main</span></span><span class="l"><span class="k">-----&gt;</span> Receiving push for <span class="cmd">myapp</span></span><span class="l"><span class="k">-----&gt;</span> Detecting runtime: <span class="cmd">node</span></span><span class="l"><span class="k">-----&gt;</span> Installing dependencies <span class="dim">(npm ci)</span></span><span class="l"><span class="k">-----&gt;</span> Launching <span class="cmd">web.1</span> under supervisor</span><span class="l"><span class="k">-----&gt;</span> Writing nginx config <span class="dim">+ TLS</span></span><span class="l"><span class="ok">=====&gt;</span> Deployed <span class="url">https://myapp.example.com</span> <span class="cur"></span></span></code></pre>
</div>

</div>

<div class="grid cards" markdown>

-   :material-rocket-launch:{ .lg .middle } **Git push to deploy**

    ---

    Push your code and Riku builds, configures, and runs it — exactly like Heroku, on a box you own.

    [:octicons-arrow-right-24: Quick Start](quick-start.md)

-   :material-language-rust:{ .lg .middle } **Single binary, no runtime**

    ---

    One ~30 MB binary. No Docker required, no Python runtime, no daemons you didn't ask for.

    [:octicons-arrow-right-24: Installation](installation.md)

-   :material-database:{ .lg .middle } **Managed addons**

    ---

    Attach Postgres, Redis, or SQLite to an app with two commands. Addons ship as plugins.

    [:octicons-arrow-right-24: Addons](addons.md)

-   :material-backup-restore:{ .lg .middle } **Backup, restore & rollback**

    ---

    Snapshot an app to a tarball, restore it anywhere, and roll back to any previous release.

    [:octicons-arrow-right-24: Backup & Restore](backup-restore.md)

-   :material-view-dashboard:{ .lg .middle } **Embedded dashboard**

    ---

    A read-only web UI is baked into the binary — app list, live logs, and history.

    [:octicons-arrow-right-24: Dashboard](dashboard.md)

-   :material-puzzle:{ .lg .middle } **Plugins & marketplace**

    ---

    Extend Riku with runtime, addon, router, and notifier plugins — installable by name.

    [:octicons-arrow-right-24: Marketplace](marketplace.md)

</div>

---

## Deploy in three steps

=== "1. Initialize the server"

    ```bash
    # On your server, as the deploy user
    riku init
    riku setup ssh ~/.ssh/id_rsa.pub
    ```

=== "2. Add a Procfile to your app"

    ```bash
    # Procfile
    web: node server.js
    ```

=== "3. Push"

    ```bash
    git remote add riku deploy@your-server:myapp
    git push riku main
    ```

Riku detects the runtime, installs dependencies, starts the process under its
supervisor, and wires up nginx. That's the whole loop.

---

## Why Riku?

| | Riku (Rust) | Original Piku (Python) |
|---|-------------|------------------------|
| Distribution | Single binary | Requires Python runtime |
| Dependencies | None at runtime | Python 3 required |
| Memory safety | Compile-time guarantees | Garbage collected |
| Footprint | ~30 MB | ~100+ MB |
| Startup | Fast | Slower |

Riku is a complete Rust port of [Piku](https://github.com/piku/piku) and competes
with Piku, Dokku, and CapRover — winning on the single-binary, no-Docker story.
It runs on one small box: 1 core, 256 MB RAM is enough.

!!! tip "Stands on the shoulders of giants"
    Thanks to the Piku team for creating the original micro-PaaS that inspired
    this port.

---

## Supported runtimes

=== "Python"
    ```bash
    # requirements.txt + Procfile
    web: gunicorn app:app
    ```

=== "Node.js"
    ```bash
    # package.json + Procfile
    web: node server.js
    ```

=== "Go"
    ```bash
    # go.mod + Procfile
    web: ./server
    ```

=== "Ruby"
    ```bash
    # Gemfile + Procfile
    web: bundle exec puma
    ```

=== "Rust"
    ```bash
    # Cargo.toml + Procfile
    web: ./target/release/myapp
    ```

Java, Clojure, and containers are supported too — see [Runtimes](runtimes.md).

---

[Ready? Deploy your first app :material-arrow-right:](quick-start.md){ .md-button .md-button--primary }
