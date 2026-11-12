# rust

A minimal Rust web app for Riku, built by the `rust-lang` runtime plugin
(detected through `Cargo.toml`). It has no dependencies: [src/main.rs](./src/README.md)
writes `Hello!` to every connection, as raw bytes with no HTTP response
line, so it is not a valid HTTP reply.

- `Cargo.toml`, `Cargo.lock`: the `riku-rust-example` package.
- `rust-toolchain.toml`: pins Rust 1.78.0.
- `Procfile`: `web: cargo run`.
- `ENV`: `PORT=4000`.
- `SCALING`: one `web` worker.

This crate is not a member of the riku workspace. Run it on its own with
`RIKU_BIND_ADDRESS=127.0.0.1 PORT=4000 cargo run` from this directory.
