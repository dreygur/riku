# login

`POST /api/login` with a JSON body `{ "password": "..." }`.

## Interface
- `POST` [route.ts:POST] (route handler)
- `dynamic` [route.ts:dynamic] (const): `"force-dynamic"`

## Invariants
- [route.ts] Returns `500` when `RIKU_DASHBOARD_PASSWORD_HASH` is unset and `401` on a wrong password; the password is checked with `verifyPassword` from `lib/password-hash.ts`
- [route.ts] Returns `429` with `Retry-After` while the client is locked out by `lib/login-throttle.ts` (fifth failure locks for 30 s, doubling to 15 min, success resets); the client is the last `X-Forwarded-For` entry, which is only trustworthy while the dashboard binds to loopback behind riku's router

## Side Effects
- [route.ts] On success sets the `riku_session` cookie: `HttpOnly`, `SameSite=Strict`, path `/`, 12-hour max age, `Secure` only when `x-forwarded-proto` (or the request URL) is https

## Verification
- Run: `nub run test:e2e`
