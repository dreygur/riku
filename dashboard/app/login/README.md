# login

`/login`: the password form. `proxy.ts` sends unauthenticated page
requests here with the original path in `?from=`.

## Interface
- `LoginPage` [page.tsx:LoginPage] (function)
- `dynamic` [page.tsx:dynamic] (const)

## Side Effects
- [page.tsx] Posts the password to `/api/login`, then `router.replace`s to `?from=` or `/`. The `from` value passes through `lib/safe-redirect.ts`, so only same-origin paths are followed.

## Verification
- Run: `nub run test:e2e`
