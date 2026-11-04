# login

`/login`: the password form. `middleware.ts` sends unauthenticated page
requests here with the original path in `?from=`.

## Interface
- `LoginPage` [page.tsx:LoginPage] (function)
- `dynamic` [page.tsx:dynamic] (const)

## Side Effects
- [page.tsx] Posts the password to `/api/login`, then `router.replace`s to `?from=` or `/`. The `from` value is not checked to be a same-site path.

## Verification
- Run: `nub run test:e2e`
