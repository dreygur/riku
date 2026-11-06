# [...path]

The same-origin proxy to the riku binary's embedded API. Every request to
`/api/riku/<path>` is forwarded to `${RIKU_API_URL}/api/<path>` (default
`http://127.0.0.1:8088`) with the query string kept.

## Interface
- `GET`, `POST`, `PUT`, `DELETE`, `PATCH` [route.ts] (route handlers): one shared handler
- `dynamic` [route.ts:dynamic] (const): `"force-dynamic"`, so responses are never cached by Next

## Invariants
- [route.ts] `Authorization: Bearer $RIKU_DASHBOARD_TOKEN` is attached server side when the token is set, so the browser never sees it
- [route.ts] Request bodies are streamed through with their original `content-type`, so multipart uploads (the restore archive) are not rewritten or buffered
- [route.ts] Response bodies pass through verbatim, which keeps SSE streams (logs, metrics) live

## Side Effects
- [route.ts] Makes an HTTP request upstream; returns `502 supervisor unreachable` when it fails

## Verification
- Run: `nub run test:e2e`
