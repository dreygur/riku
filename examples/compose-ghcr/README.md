# Compose + GHCR auto-update demo

A minimal app for manually exercising riku's compose/GHCR support end to end:
detecting a compose app, pulling a private GHCR image, and auto-updating a
service when you push a new image tag: with no `git push riku`.

This is a real demo app you `git push` to an actual riku server that has
Docker or Podman installed: not a self-contained sandbox (see `tests/demo/`
for a compose file that boots a full riku instance you can push to).

## 1. Build and push the demo image to your own GHCR

```bash
cd examples/compose-ghcr/app

export RIKU_GHCR_USERNAME=<your-github-username>
docker build --build-arg VERSION=v1 -t ghcr.io/$RIKU_GHCR_USERNAME/riku-compose-demo:latest .

# A classic PAT with `write:packages` (or `docker login` interactively) works too.
echo "$RIKU_GHCR_TOKEN" | docker login ghcr.io -u "$RIKU_GHCR_USERNAME" --password-stdin
docker push ghcr.io/$RIKU_GHCR_USERNAME/riku-compose-demo:latest
```

## 2. Point compose.yml at your image

Edit `compose.yml` (one directory up from `app/`) and replace `RIKU_GHCR_USERNAME`
with the same username you used above.

The web service publishes its container port, `"8080"`. Riku picks a free
host port on 127.0.0.1 for each copy and routes to it, so the compose file
names no host port. Riku refuses a port entry that names a host IP (such as
`"127.0.0.1:8080:8080"`) with "use the riku.public label instead of a host
IP". Published ports bind loopback by default; set the service label
`riku.public: "true"` only if the port must listen on all interfaces.

```yaml
services:
  web:
    image: ghcr.io/<your-github-username>/riku-compose-demo:latest
    ports:
      - "8080"
```

## 3. Deploy it to riku

```bash
cd examples/compose-ghcr
riku apps create compose-demo
git init -q && git add -A && git commit -q -m "compose demo v1"
git remote add riku riku@<your-riku-server>:compose-demo
git push riku main
```

If the image is private, set credentials **before** the push so the first
build can log in and pull:

```bash
riku settings set compose-demo secrets.ghcr_username=$RIKU_GHCR_USERNAME secrets.ghcr_token=$RIKU_GHCR_TOKEN
```

Confirm it's up by requesting the app through the router, at the domain you set
with `riku settings set compose-demo routing.domains=<your-domain>`:

```bash
curl http://<your-domain>
# version=v1 time=2026-07-07T...
```

## 4. Turn on auto-update

```bash
riku settings set compose-demo containers.watch_services=web
```

The supervisor now re-checks this service's image every 60 seconds.

## 5. Push a new image and watch it update itself

```bash
cd app
docker build --build-arg VERSION=v2 -t ghcr.io/$RIKU_GHCR_USERNAME/riku-compose-demo:latest .
docker push ghcr.io/$RIKU_GHCR_USERNAME/riku-compose-demo:latest
```

Wait up to 60 seconds, no `git push` needed:

```bash
curl http://<your-domain>
# version=v2 time=2026-07-07T...
```

That's the whole feature: riku noticed the `latest` tag moved, pulled it, and
recreated just the `web` service: the rest of the app (and any other
services in the same compose file) untouched.

## Cleanup

```bash
riku destroy compose-demo
```

See [docs/docs/runtimes.md](../../docs/docs/runtimes.md#container) and
[docs/docs/env.md](../../docs/docs/env.md#container-compose-settings) for
the full reference. The app image itself is described in
[app/README.md](app/README.md).
