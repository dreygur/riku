# lib

Shared client and server code for the dashboard.

## Interface
- `api` [api.ts:api] (const): every backend call, all through the same-origin `/api/riku` proxy; `fmtBytes`, `fmtDur` [api.ts] (const) format sizes and durations
- `SESSION_COOKIE`, `SESSION_TTL_MS` [auth.ts] (const), `createSessionToken`, `verifySessionToken` [auth.ts] (function): the `riku_session` HMAC token, Web Crypto only so it runs in both the Edge middleware and Node route handlers
- `hashPassword`, `verifyPassword` [password-hash.ts] (function): scrypt, Node only; do not import from `middleware.ts`
- `safeRedirectPath` [safe-redirect.ts:safeRedirectPath] (function): the login page's `from` target, kept only when it is a same-origin path, otherwise `/`
- `DotKind` [status.ts:DotKind] (type), `statusMeta` [status.ts:statusMeta] (function): process status to dot colour and label
- [types.ts]: the API response types (`ProcStatus`, `Worker`, `RouteState`, `AppState`, `RikuState`, `Release`, `DoctorCheck`, `AddonInstance`, `PluginsList`, `UiPanel`, `MarketplaceSource`, `MarketplaceHit`, `TrustKey` and others) and small helpers (`domainOf`, `httpsOf`, `routeBadge`, `isBusy`)
- `usePendingRun` [use-pending-run.ts:usePendingRun] (function): tracks which async action is in flight so its control can show a pending state
- `cn` [utils.ts:cn] (function): class name merge

## Invariants
- [password-hash.ts] The stored value is `${saltHex}:${hashHex}`; the plaintext password is never persisted
- [auth.ts] The HMAC key is derived from `SHA-256(RIKU_DASHBOARD_PASSWORD_HASH)`, never from the plaintext

## Side Effects
- [api.ts] Makes HTTP requests to `/api/riku/*`

## Verification
- Run: `nub run test:unit` and `nub run test:e2e`
