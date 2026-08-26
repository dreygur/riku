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

export GHCR_USERNAME=<your-github-username>
docker build --build-arg VERSION=v1 -t ghcr.io/$GHCR_USERNAME/riku-compose-demo:latest .

# A classic PAT with `write:packages` (or `docker login` interactively) works too.
echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USERNAME" --password-stdin
docker push ghcr.io/$GHCR_USERNAME/riku-compose-demo:latest
```

## 2. Point compose.yml at your image

Edit `compose.yml` (one directory up from `app/`) and replace `GHCR_USERNAME`
with the same username you used above.

## 3. Deploy it to riku

```bash
cd examples/compose-ghcr
riku apps create compose-demo
git init -q && git add -A && git commit -q -m "compose demo v1"
git remote add riku deploy@<your-riku-server>:compose-demo
git push riku main
```

If the image is private, set credentials **before** the push so the first
build can log in and pull:

```bash
riku config set compose-demo GHCR_USERNAME=$GHCR_USERNAME GHCR_TOKEN=$GHCR_TOKEN
```

Confirm it's up:

```bash
curl http://<your-riku-server>:8080
# version=v1 time=2026-07-07T...
```

## 4. Turn on auto-update

```bash
riku config set compose-demo RIKU_WATCH_SERVICES=web
```

The supervisor now re-checks this service's image every 60 seconds.

## 5. Push a new image and watch it update itself

```bash
cd app
docker build --build-arg VERSION=v2 -t ghcr.io/$GHCR_USERNAME/riku-compose-demo:latest .
docker push ghcr.io/$GHCR_USERNAME/riku-compose-demo:latest
```

Wait up to 60 seconds, no `git push` needed:

```bash
curl http://<your-riku-server>:8080
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
the full reference.
