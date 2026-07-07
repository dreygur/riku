# riku sandbox

A single container running a full, working riku instance — real supervisor,
real dashboard, real nginx, real sshd — with 5 demo apps already deployed, so
you can open the dashboard and poke at a populated instance immediately.

Unlike `tests/e2e/` (scripted, tears itself down) or `tests/stress/`
(load/chaos testing), this one is meant to stay running so a human can look
at it.

## Run it

```bash
cargo build --release   # compose.yml reuses this binary rather than
                         # recompiling the whole workspace in a Docker stage
docker compose -f tests/demo/compose.yml up --build
```

or just `./run_demo.sh` (does the `cargo build --release` for you, then
brings the container up in the background and prints the URLs).

## What's deployed

| App | Runtime | Notable |
|---|---|---|
| `hello-node` | Node.js | plain `http` module, no dependencies |
| `hello-python` | Python | bound to a `sqlite-volume` addon instance (`demodb`) — its response shows `SQLITE_PATH` |
| `hello-ruby` | Ruby | stdlib `TCPServer`, no gems/bundler needed |
| `hello-go` | Go | stdlib `net/http` |
| `hello-worker` | Python | multi-process: `web` + `worker` + a once-a-minute `cron` job |

Each app source lives under `apps/<name>/` and gets pushed to its own bare
repo inside the container at boot — the real `git push` / post-receive-hook
path, just invoked over a local filesystem path instead of `ssh://` so no
keypair is needed for the initial deploy.

## Open it

`*.localhost` resolves to `127.0.0.1` on its own on virtually every modern
OS/browser — no `/etc/hosts` edit needed.

- Dashboard: http://dashboard.localhost:8080
- Apps: http://hello-node.localhost:8080, `hello-python`, `hello-ruby`,
  `hello-go`, `hello-worker.localhost:8080`

## Push more apps / redeploy the demo ones yourself

```bash
ssh-keygen -t ed25519 -f tests/demo/.ssh-bootstrap/id_ed25519 -N ""
docker compose -f tests/demo/compose.yml up --build -d   # re-create so the key gets imported
ssh -i tests/demo/.ssh-bootstrap/id_ed25519 -p 2222 riku@127.0.0.1 apps create myapp
git push ssh://riku@127.0.0.1:2222/myapp main
```

## Stop it

```bash
./stop_demo.sh   # or: docker compose -f tests/demo/compose.yml down
```

## Logs

```bash
docker compose -f tests/demo/compose.yml logs -f
```
