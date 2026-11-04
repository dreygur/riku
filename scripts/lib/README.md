# lib

Shell helpers sourced by the scripts in `scripts/`.

- `dashboard-token.sh`: `dashboard_token` prints the dashboard token from `${XDG_CONFIG_HOME:-$HOME/.config}/riku/dashboard-token`, creating it with `openssl rand -hex 32` (mode 600) on first use. `dashboard-tunnel.sh` starts the backend with it and `dashboard-web.sh` sends it, so the token is never copied between terminals.

## Verification
- Run: `shellcheck scripts/lib/dashboard-token.sh`
