---
title: "CodeQL flags inline format captures and the deploy login name; dismiss them, don't code around them"
author: Rakibul
date: 2026-10-29
branch: feat/caddy-router
cost: { minutes: 1, tokens: 1377 }
id: l-34dc54c0
scope: repo
status: active
replaces: l-f431b0f7
src: agent
touches:
  - "crates/riku-caddy/src/site.rs#render_site"
  - crates/riku-cli/src/setup/root_init.rs
  - "crates/riku-util/src/display.rs#echo"
---

## Decided
Two CodeQL alerts on the Caddy router branch are false positives and get dismissed, not coded around.
- rust/unused-variable on a parameter used only through an inline format capture (`{app}` inside `format!`). CodeQL's Rust extractor does not see inline captures.
- rust/cleartext-logging on the deploy account login name. Root init prints it on purpose to the admin running it; a login name is not a secret. CodeQL treats any user name field as private data.

## Tried and ruled out
Rewriting the code to dodge the scanner (positional format args, hiding the user name). It changes working code only to silence a warning.

## Watch out
Dismissing alerts through `gh api -X PATCH .../code-scanning/alerts/<n>` needs the owner to run it; agent auto mode blocks it as an external write. To see why CodeQL flagged a flow, download the SARIF with `gh api -H "Accept: application/sarif+json" repos/dreygur/riku/code-scanning/analyses/<id>` and read `codeFlows`.

In the code:
- crates/riku-caddy/src/site.rs#render_site
- crates/riku-cli/src/setup/root_init.rs (the "Initializing Riku for deploy user" line)
- crates/riku-util/src/display.rs#echo (the sink)
