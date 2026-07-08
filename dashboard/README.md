# Riku Dashboard

Next.js 16 + React 19 control plane for the Riku supervisor.

The dashboard runs a same-origin server-side proxy at `/api/riku/[...path]`
(`app/api/riku/[...path]/route.ts`) that forwards every call to the Rust
backend (`crates/riku-dashboard`, default `http://127.0.0.1:8088`), attaching
the backend's bearer token server-side — the browser never sees it.

That backend token alone is **not** enough to expose this dashboard publicly:
it protects the backend process from being hit directly by something other
than this proxy, but by itself the *frontend* had no login of its own. A
session-based login layer sits in front of it for that reason (below).

## Security configuration

### Backend token (`RIKU_DASHBOARD_TOKEN`)

The proxy attaches `Authorization: Bearer $RIKU_DASHBOARD_TOKEN` to every
request it forwards to the Rust backend (`app/api/riku/[...path]/route.ts`).
If unset, no `Authorization` header is sent — matches the backend's own
loopback-only default (see [Dashboard](../docs/docs/dashboard.md)).

### Frontend login (`RIKU_DASHBOARD_PASSWORD_HASH`)

A single shared password gates the whole frontend — every page and the
`/api/riku/*` proxy — via a signed session cookie:

- **`middleware.ts`** checks a session cookie (`riku_session`) on every
  request. No `RIKU_DASHBOARD_PASSWORD_HASH` set → auth is fully bypassed
  (local/dev default). Unauthenticated pages redirect to `/login`;
  unauthenticated API calls get `401` JSON instead.
- **`lib/password-hash.ts`** — the password is never stored in plaintext.
  `RIKU_DASHBOARD_PASSWORD_HASH` holds a `scrypt` salt+hash
  (`${saltHex}:${hashHex}`); generate it with:

  ```bash
  nub run hash-password -- 'your-chosen-password'
  ```
- **`lib/auth.ts`** — the session cookie is an HMAC-SHA256 token
  (`${expiryMs}.${signature}`), keyed off `SHA-256(RIKU_DASHBOARD_PASSWORD_HASH)`
  — the stored hash itself, never the plaintext — via Web Crypto
  (`crypto.subtle`), so the same code runs in both the Edge middleware and the
  Node `/api/login` route. 7-day expiry, `HttpOnly`, `SameSite=Strict`,
  `Secure` only when the request actually arrived over HTTPS (checked via
  `x-forwarded-proto`, so it still works during local/plain-HTTP testing).

### Environment variables

| Variable | Default | Purpose |
| --- | --- | --- |
| `RIKU_DASHBOARD_TOKEN` | *(unset)* | Bearer token the proxy attaches to backend calls. |
| `RIKU_DASHBOARD_PASSWORD_HASH` | *(unset → login disabled)* | `scrypt` salt+hash gating the whole frontend. Generate with `nub run hash-password`. |
| `RIKU_API_URL` | `http://127.0.0.1:8088` | Where the proxy forwards to. |

## Running

```bash
RIKU_DASHBOARD_TOKEN=$(openssl rand -hex 32) \
RIKU_DASHBOARD_PASSWORD_HASH=$(nub run hash-password -- 'your-chosen-password' | tail -1 | cut -d= -f2) \
next start -H 127.0.0.1
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
