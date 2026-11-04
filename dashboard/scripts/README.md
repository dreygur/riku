# scripts

- `hash-password.ts`: `nub run hash-password -- '<password>'` prints the `RIKU_DASHBOARD_PASSWORD_HASH=<salt>:<hash>` line to put in the dashboard's environment.
- `audit-dashboard.ts`: `nub run audit:dashboard`, an audit of live backend routes. It is stale: it targets a Hono catch-all (`/api/health`, `/api/env/:app`, `/api/metrics/apps`) and `app/api/logs/stream/route.ts`, none of which exist in this dashboard any more, and expects the Rust health server on port 9091 by default.

## Verification
- Run: `nub run hash-password -- test`
