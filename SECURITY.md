# Security Model

## Trust Boundary

Riku follows the same trust model as Heroku and Piku: **anyone with git push access can execute arbitrary code on the server**. Procfile commands (web, worker, preflight, release) run via `sh -c` as the riku user with no sandboxing.

This is by design. A PaaS that executes user-provided Procfiles inherently grants shell access.

## Operator Responsibilities

- **SSH access = server access.** Only add trusted users' SSH keys to `authorized_keys`.
- **Run riku under a dedicated unprivileged user** (e.g., `riku` or `piku`). Never run as root.
- **Restrict the riku user's capabilities** — no sudo, limited filesystem access.
- **For untrusted workloads**, use container deployments with Docker/Podman to add isolation.

## Input Validation

Riku validates inputs at system boundaries:

| Input | Validation |
|-------|-----------|
| App names | Alphanumeric, dots, underscores, hyphens only. Path traversal (`..`) rejected. |
| Plugin names | No path separators (`/`, `\`) or traversal sequences. |
| Environment variables | Control characters stripped. Nginx-bound values reject `;`, `{`, `}`, newlines. |
| Nginx configs | Template injection prevented by sanitizing ENV values. Generated configs validated by `nginx -t`. |
| Symlink targets | Resolved paths verified to stay within the riku directory tree before destructive operations. |

## Process Isolation

- Each app's processes run in their own process group for signal handling.
- Worker processes are monitored by the supervisor with health checks and automatic restarts.
- Log files are isolated per-app under `~/.riku/logs/<app>/`.
- The supervisor uses file-based advisory locking to prevent concurrent write corruption.

## Reporting Vulnerabilities

If you discover a security vulnerability, please open an issue at https://github.com/dreygur/riku/issues with the `security` label.
