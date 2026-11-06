# quickstart

`riku quickstart`: scaffolds a runnable sample app (python or node) in the
current directory and prints the exact `git remote add` and `git push` lines.
It runs on the developer's machine, independent of the server install.

## Interface
- `cmd_quickstart` [mod.rs:cmd_quickstart] (fn)
- `ScaffoldFile` [scaffold.rs:ScaffoldFile] (struct)
- `Runtime` [scaffold.rs:Runtime] (enum), with `parse`, `label` and `files`

## Invariants
- [mod.rs] The app name goes through `validate_app_name` before it becomes a directory, and an existing directory is refused

## Side Effects
- [mod.rs] Writes the scaffold files into `./<name>`
- [mod.rs] Runs `git init` and an initial commit through `std::process::Command` when git is installed

## Verification
- Run: `cargo test -p riku-remote`
