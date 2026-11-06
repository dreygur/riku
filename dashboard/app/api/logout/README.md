# logout

`POST /api/logout`.

## Interface
- `POST` [route.ts:POST] (route handler)
- `dynamic` [route.ts:dynamic] (const): `"force-dynamic"`

## Side Effects
- [route.ts] Deletes the `riku_session` cookie. The token itself is not revoked server side; a copied token stays valid until it expires.

## Verification
- Run: `nub run test:e2e`
