# riku-remote

Laptop side behaviour shared by the `riku` and `rk` binaries: the named server
list and command forwarding, the dashboard client, client plugins, quickstart
scaffolding and plugin routing. It depends on no server crate, so a client
binary built on it stays small.

- [src](./src/README.md)

Run the tests with `cargo test -p riku-remote`.
