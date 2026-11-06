# riku-client

Builds the `rk` binary, the laptop only client. It parses the same command
tree as `riku`, then runs a local command, forwards to a server over ssh, or
refuses. It never runs server code and is never the ssh forced command.

- [src](./src/README.md)
- `tests/deps.rs`: guards that neither riku-client nor riku-args links a server side crate or the HTTP stack.
- `tests/rk_tests.rs`: runs the built `rk` binary against a throwaway config and home directory.

Run the tests with `cargo test -p riku-client`.
