# dashboard

`riku dashboard` with a server selected: runs that server's embedded dashboard
for this session only and opens it in the local browser. One ssh master to the
server's admin login carries every step: a sudo call starts a shell as the
deploy user, which reads the session token on stdin and starts
`riku dashboard --local --bind 127.0.0.1:0`; the local side forwards a port to
it and opens the browser.

## Interface
- `run_remote` [mod.rs:run_remote] (fn): the entry point
- `check_remote_flags` [flags.rs:check_remote_flags] (fn, re-exported): rejects flags only honored when serving locally
- `BOUND_PREFIX` [remote_script.rs:BOUND_PREFIX] (const, re-exported); `SUDO_PROMPT`, `READY_MARKER`, `script`, `script_command`, `parse_bound` [remote_script.rs]
- `pick_local_port`, `dashboard_url`, `open` [browser.rs] (fn)
- `ended_message` [ended.rs:ended_message] (fn)
- `Event` [events.rs:Event] (enum), `read_stdout`, `read_stderr` [events.rs] (fn)
- `Action`, `Stage` [handshake.rs] (enum), `Handshake` [handshake.rs] (struct): pure state machine for prompt, marker and port lines
- `install`, `requested` [interrupt.rs] (fn): Ctrl-C, SIGHUP and SIGTERM set a flag instead of killing the process
- `RemoteDashboard` [session.rs:RemoteDashboard] (struct), with `spawn` and `start`
- `control_path` [ssh_master.rs:control_path] (fn), `SshControl` [ssh_master.rs:SshControl] (struct)
- `stop_child`, `quiet_ssh` [stop.rs] (fn), `Master` [stop.rs:Master] (struct)
- `Secret` [sudo.rs:Secret] (struct), `prompt_password` [sudo.rs:prompt_password] (fn)

## Invariants
- [mod.rs] The token is fresh per session from the OS CSPRNG and reaches the server only on stdin, never in an argument list there
- [handshake.rs, remote_script.rs] The token is sent only after `READY_MARKER`, and a second sudo prompt aborts, so a wrong password costs one attempt
- [ssh_master.rs] The control socket lives in a private per-session temp dir; the admin address is a validated `SshAddress` after `--`; the local forward binds `127.0.0.1`
- [sudo.rs] The password is read from `/dev/tty` with echo off and zeroed on drop
- [stop.rs] ssh processes are stopped with SIGTERM after a short wait, never SIGKILL

## Side Effects
- [browser.rs] Binds a local port to pick it, and spawns the system browser opener; the URL, token included, is briefly visible in that process's argv
- [interrupt.rs] Installs signal handlers through `nix::sys::signal::sigaction`
- [session.rs, ssh_master.rs, stop.rs] Spawns and signals `ssh` processes
- [sudo.rs] Switches the terminal to no-echo and restores it

## Verification
- Run: `cargo test -p riku-remote`
