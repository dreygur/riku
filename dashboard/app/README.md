# app

The Next.js App Router tree. Every page is a client page that loads its data
through `api` from `lib/api.ts`, which goes through the `/api/riku` proxy.

## Interface
- `metadata` [layout.tsx:metadata] (const), `RootLayout` [layout.tsx:RootLayout] (function): the shell around every page
- `Overview` [page.tsx:Overview] (function): `/`, an `AppCard` per app from `api.state`
- [addons](./addons/README.md): `/addons`
- [api](./api/README.md): route handlers (login, logout, the backend proxy)
- [apps](./apps/README.md): `/apps/[app]`
- [doctor](./doctor/README.md): `/doctor`
- [login](./login/README.md): `/login`
- [marketplace](./marketplace/README.md): `/marketplace`
- [metrics](./metrics/README.md): `/metrics`
- [plugins](./plugins/README.md): `/plugins`

## Verification
- Run: `nub run test:e2e`
