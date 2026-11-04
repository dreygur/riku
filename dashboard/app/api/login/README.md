# login

`POST /api/login` with a JSON body `{ "password": "..." }`.

## Interface
- `POST` [route.ts:POST] (route handler)
- `dynamic` [route.ts:dynamic] (const): `"force-dynamic"`

## Invariants
- [route.ts] Returns `500` when `RIKU_DASHBOARD_PASSWORD_HASH` is unset and `401` on a wrong password; the password is checked with `verifyPassword` from `lib/password-hash.ts`
- [route.ts] There is no rate limit or lockout on attempts; scrypt's cost is the only brake

## Side Effects
- [route.ts] On success sets the `riku_session` cookie: `HttpOnly`, `SameSite=Strict`, path `/`, 7-day max age, `Secure` only when `x-forwarded-proto` (or the request URL) is https

## Verification
- Run: `nub run test:e2e`
