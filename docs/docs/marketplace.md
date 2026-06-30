# Marketplace

The marketplace lets you **discover and install plugins by name** from git
repositories that index them — the difference between "extensible" and an actual
ecosystem. Riku adopts a git-native marketplace shape (like Claude Code's),
hardened for server-side execution with signature verification and capability
declarations.

The Riku repository is itself a marketplace, indexing the official plugins:
`postgres`, `redis`, `sqlite-volume` (addons), `caddy-router` (router), and
`webhook-notify` (notifier).

## Register a marketplace

A marketplace is a git repo with a `marketplace.toml` index. Add one, and Riku
clones it locally:

```bash
riku plugins marketplace add github:dreygur/riku
```

```text
riku plugins marketplace add     # Register and clone a marketplace
riku plugins marketplace list    # List registered marketplaces
riku plugins marketplace remove  # Remove a registered marketplace
```

## Search

```bash
# Search names and descriptions across registered marketplaces
riku plugins search postgres

# Empty query lists everything available
riku plugins search
```

## Install by name

```bash
# Install the latest match by name
riku plugins add postgres

# Disambiguate when registered in multiple marketplaces
riku plugins add postgres@official
```

The spec is `name` or `name@marketplace`.

## Managing installed plugins

```text
riku plugins list      # List installed plugin bundles
riku plugins remove    # Remove an installed plugin bundle
riku plugins doctor    # Validate API compatibility + integrity
riku plugins install   # Install from a local path or git URL (no marketplace)
```

## Trust & signatures

Because plugins execute on your server, Riku supports **publisher signing** so
you only install code from keys you trust.

### As an operator: trust publishers

```text
riku plugins trust add     # Trust a publisher's public key
riku plugins trust list    # List trusted publisher keys
riku plugins trust remove  # Remove a trusted publisher key
```

### As an author: sign your plugin

```text
riku plugins keygen        # Generate an Ed25519 signing keypair
riku plugins sign          # Sign a plugin bundle's entry with a secret key
```

Riku verifies signatures against trusted keys and validates bundle integrity
(checksums) on install. Run `riku plugins doctor` to re-check installed bundles.

!!! warning "Server-side execution"
    Marketplace plugins run with the privileges of the Riku deploy user. Only
    register marketplaces and trust keys you control or have vetted. Capability
    declarations in a plugin's manifest (network, filesystem writes) tell you
    what a plugin intends to do before you install it.

## Authoring a plugin

Scaffold a new bundle skeleton, implement the protocol verbs for its type
(runtime, addon, router, or notifier), sign it, and publish it in a marketplace
repo:

```bash
riku plugins scaffold
```

See [Plugin Bundles](plugin-bundles.md), [Addons](addons.md), and the
[Plugin Protocol](https://github.com/dreygur/riku/blob/main/PLUGIN_PROTOCOL.md)
for the full contract.
