# webhook-notify

A minimal Riku **notifier** plugin (Plugin Protocol v1). It subscribes to
lifecycle events and POSTs each event envelope to a webhook.

A notifier is not a special plugin kind — it is simply an **event subscriber**:
a bundle whose `riku-plugin.toml` declares an `[events]` block. The kernel runs
`entry` with the verb `on_event` and the event JSON on stdin for each subscribed
event.

## Install (local/dev)

Copy the bundle into the plugins directory:

```sh
cp -r examples/plugins/webhook-notify ~/.riku/plugins/
chmod +x ~/.riku/plugins/webhook-notify/bin/on-event
```

Set the webhook URL in the app environment:

```sh
riku config set <app> WEBHOOK_URL=https://hooks.example.com/...
```

On the next deploy you will receive a POST for `deploy.requested`,
`build.finished`, and `deploy.finished`.

## Contract

- **Mode:** `observe` — failures are logged, never block a deploy.
- **Input:** one JSON line on stdin per event, e.g.
  `{"api":1,"event":"deploy.finished","ts":"...","app":"myapp","data":{...}}`.
- **Capabilities:** `network = true` (declared in the manifest).

See `PLUGIN_PROTOCOL.md` for the full event catalog and contract.
