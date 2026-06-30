# Riku Dashboard

Next.js 16 + Hono 4 + React 19 control plane for the Riku supervisor.

The dashboard runs a server-side API proxy at `/api/*` (`app/api/[[...route]]/route.ts`)
that forwards to the Rust supervisor. Mutating routes deputize the Rust control
token (`~/.riku/control.token`) onto upstream `/control/*` calls, so the
browser-facing side must defend against confused-deputy / CSRF abuse.

## Security configuration

Mutating routes (the `control/*` router and `env` PUT/DELETE) are protected by:

1. **Operator token** — every mutating request must carry the header
   `x-riku-dashboard-token` whose value matches `RIKU_DASHBOARD_TOKEN`
   (constant-time compared server-side). Read-only GETs
   (health/metrics/network/logs) stay unauthenticated.
2. **Same-origin / CSRF check** — the request `Origin` must equal
   `RIKU_DASHBOARD_ORIGIN`; Origin-less requests are accepted only with
   `Sec-Fetch-Site: same-origin`. Cross-site requests are rejected `403`.
3. **Locked CORS** — `Access-Control-Allow-Origin` echoes only
   `RIKU_DASHBOARD_ORIGIN`, never `*`.

### Environment variables

| Variable                | Default                  | Purpose |
| ----------------------- | ------------------------ | ------- |
| `RIKU_DASHBOARD_TOKEN`  | *(unset → fail closed)*  | Shared secret required on every mutating request. If unset, mutating routes return `503 "dashboard token not configured"` — they never default to open. |
| `RIKU_DASHBOARD_ORIGIN` | `http://127.0.0.1:3000`  | The single allowed browser origin for CORS + CSRF. Must match the same env var on the Rust side (`src/supervisor/health/mod.rs`). |

The dashboard's own client JS obtains the token same-origin from `GET /api/csrf`
(unreadable cross-site) and sends it on mutating calls — the token is never
embedded in the JS bundle and never exposed via `NEXT_PUBLIC_*`.

## Running

Bind the server to loopback so the dashboard is not reachable from other hosts:

```bash
RIKU_DASHBOARD_TOKEN=$(openssl rand -hex 32) next start -H 127.0.0.1
```

## Tooling

This project uses [nub](https://nubjs.com) as its package manager (declared via
`packageManager` in `package.json`; the lockfile is `lock.yaml`). Install it once:

```bash
npm install -g @nubjs/nub   # or: brew install nubjs/tap/nub, mise use -g nub
```

Then install dependencies:

```bash
nub install                 # nub ci / nub install --frozen-lockfile in CI
```

## Scripts

```bash
nub run dev         # next dev --turbopack
nub run build       # next build
nub run typecheck   # tsc --noEmit
nub run test:e2e    # playwright test
```
