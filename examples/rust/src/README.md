# rust (src)

- `main.rs`: binds a `TcpListener` on `$RIKU_BIND_ADDRESS:$PORT` (it panics when either is unset) and writes `Hello!` to every connection. The reply is raw bytes with no HTTP status line or headers.

## Verification
- Run: `RIKU_BIND_ADDRESS=127.0.0.1 PORT=4000 cargo run` from `examples/rust`, then `nc 127.0.0.1 4000`
