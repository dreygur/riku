# addons

`/addons`: lists addon instances and creates, binds, unbinds, backs up and
destroys them.

## Interface
- `AddonsPage` [page.tsx:AddonsPage] (function): calls `api.addons`, `addonCreate`, `addonBind`, `addonUnbind`, `addonBackup` and `addonDestroy`

## Verification
- Run: `nub run test:e2e`
