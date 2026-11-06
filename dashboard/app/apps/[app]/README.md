# [app]

`/apps/[app]`: one app's status card, its environment editor and backup.

## Interface
- `AppDetail` [page.tsx:AppDetail] (function): reads `api.state`, calls `api.backup`, and renders `AppCard`, `EnvEditor`, `PageHeader` and `confirmDialog` from `components/riku`

## Verification
- Run: `nub run test:e2e`
