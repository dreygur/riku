# marketplace

`/marketplace`: marketplace sources, plugin search and install, and the
author-signature trust keyring.

## Interface
- `MarketplacePage` [page.tsx:MarketplacePage] (function): calls `api.marketSources`, `marketAdd`, `marketRemove`, `marketSearch`, `pluginInstall`, `trust`, `trustAdd` and `trustRemove`

## Verification
- Run: `nub run test:e2e`
