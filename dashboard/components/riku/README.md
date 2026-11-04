# riku

Riku-specific dashboard components, built on the primitives in `../ui`.

## Interface
- `AppCard` [app-card.tsx:AppCard] (function): one app's processes and its actions (restart, redeploy, stop, scale, rollback, per-worker restart and delete)
- `CommandMenu` [command-menu.tsx:CommandMenu] (function): the command palette for navigation and app actions
- `confirmDialog` [confirm-dialog.tsx:confirmDialog] (function), `ConfirmHost` [confirm-dialog.tsx:ConfirmHost] (function): a promise-returning confirm and the host that renders it
- `EnvEditor` [env-editor.tsx:EnvEditor] (function): reads and writes an app's env through `api.env` and `api.setEnv`
- `LogSheet` [log-sheet.tsx:LogSheet] (function): live logs over an `EventSource` on `api.logsUrl`
- `PageHeader` [page-header.tsx:PageHeader] (function)
- `Reveal` [reveal.tsx:Reveal] (function): staggered CSS entry animation, no animation runtime
- `Sparkline` [sparkline.tsx:Sparkline] (function)
- `StatusDot` [status-dot.tsx:StatusDot] (function)
- `TopNav` [top-nav.tsx:TopNav] (function): page links, plugin UI links, and logout

## Side Effects
- [top-nav.tsx] Posts to `/api/logout`
- [app-card.tsx, command-menu.tsx, env-editor.tsx] Call mutating endpoints through `lib/api.ts`

## Verification
- Run: `nub run test:e2e`
