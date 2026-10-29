---
id: 5250752f-06ad-4fc3-80a8-702911af219c
title: "CodeQL flags inline format captures and the deploy login name as false positives"
author: "Rakibul Yeasin"
date: "2026-10-29"
scope: "project"
cost_minutes: 10
cost_tokens: 209004
touches:
  - "crates/riku-caddy/src/site.rs#render_site"
  - "crates/riku-cli/src/setup/root_init.rs"
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
