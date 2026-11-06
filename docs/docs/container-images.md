# Build images off the server

A small board does not need a build toolchain. Build the image on your laptop
or in CI, push it to a registry, and name it in the compose file with
`image:`. Riku pulls it on deploy, the same as any other `image:` service.
`git push` still delivers the compose file and everything else in the repo.

## Build for the server's platform

The image has to match the server's CPU. On the server:

```bash
uname -m
```

`x86_64` is `linux/amd64`. `aarch64` is `linux/arm64`. A 32 bit Raspberry Pi
OS reports `armv7l` (`linux/arm/v7`) or `armv6l` (`linux/arm/v6`).

Then build and push from your laptop or a CI job, with `docker buildx`:

```bash
docker buildx build --platform linux/arm64 -t ghcr.io/me/myapp:1 --push .
```

Use a new tag for every build, so the compose file says which build runs.

## Name the image in compose

```yaml
# compose.yml
services:
  web:
    image: ghcr.io/me/myapp:1
    ports:
      - "8080"
```

Commit it and `git push` as usual. See [Compose apps](runtimes.md#compose-apps)
for how riku runs the services.

## Log in to GHCR

A public image needs nothing. For a private one, give riku a username and a
token with the `read:packages` scope. Set both or neither:

```bash
riku config set myapp RIKU_GHCR_USERNAME=me RIKU_GHCR_TOKEN=ghp_xxx
```

Riku runs `docker login ghcr.io` with these before it pulls. See
[GHCR authentication](runtimes.md#ghcr-authentication) for where the login is
stored. Other registries are not logged in by riku.

## Ship a new tag

Change the tag in `compose.yml`, commit, and push:

```bash
git commit -am "Deploy myapp:2" && git push
```

To pull again without changing the file, run `riku deploy myapp`.

## A registry on the server

A host with no outside registry can run one itself. On the server:

```bash
docker run -d --name registry --restart unless-stopped \
  -p 127.0.0.1:5000:5000 registry:2
```

Use `localhost:5000/myapp:1` as the image name in `compose.yml`. Docker allows
plain HTTP to a `localhost` registry, so the server can pull it. With Podman,
check that your version does the same before relying on this.

The registry listens on loopback only, so the machine that builds has to reach
it another way. The riku ssh key is restricted to the riku command and cannot
forward ports. Use a separate ssh login for the tunnel, then push through it:

```bash
ssh -N -L 5000:localhost:5000 me@edge.example &
docker buildx build --platform linux/arm64 -t localhost:5000/myapp:1 --push .
```

This registry has no authentication. Do not bind it to a public address.
