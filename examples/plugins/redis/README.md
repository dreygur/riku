# redis

A managed **Redis** addon (`addon` seam, `PLUGIN_PROTOCOL.md` §6.1). Each
instance is an isolated tenant: a per-instance Redis **ACL user** (Redis ≥6)
scoped to its own key namespace.

## Requirements

- Redis ≥6 reachable from the host
- `redis-cli` on the host
- Admin access: defaults to local `localhost:6379`; override with `REDISHOST` /
  `REDISPORT`, and `REDISCLI_AUTH` for an admin password

## Use

```sh
riku plugins install ./examples/plugins/redis
riku addon create redis cache
riku addon bind cache <app>
```

On `bind`, the app's ENV receives:

| Var | Meaning |
|---|---|
| `REDIS_URL` | `redis://<ident>:<pass>@host:port/0` |
| `REDIS_KEY_PREFIX` | `<ident>:` — the namespace the tenant is allowed to use |

## Isolation model

Tenants are separated by **ACL key-pattern scoping** (`~<ident>:*`), not by
separate Redis databases. The ACL user can only touch keys under its own
prefix, so **your app must namespace its keys with `REDIS_KEY_PREFIX`**. Most
clients support a key-prefix option for exactly this.

## Caveats

- `backup` uses `redis-cli --rdb`, which dumps the **whole** keyspace (Redis has
  no per-user export). Restore is operator-driven.
- `deprovision` removes the ACL user but does not delete its keys; clear them
  with `redis-cli --scan --pattern '<ident>:*' | xargs redis-cli del` if needed.
