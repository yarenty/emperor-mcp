# emperor-mcp

[![CI](https://github.com/yarenty/emperor-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/yarenty/emperor-mcp/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/emperor-mcp.svg)](https://crates.io/crates/emperor-mcp)
[![docs.rs](https://img.shields.io/docsrs/emperor-mcp)](https://docs.rs/emperor-mcp)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**The enterprise MCP server framework for Rust — Streamable HTTP first, credential forwarding,
audited framing.**

Built for teams putting [Model Context Protocol](https://modelcontextprotocol.io) servers into
**production**: behind gateways, in front of load balancers, handling other people's
credentials, feeding output to models that must not mistake data for instructions.

- **Deploy like a service, not a subprocess.** Stateless Streamable HTTP: no session
  affinity, no `Mcp-Session-Id`, every request self-contained — load balancers and failover
  just work.
- **Handle credentials without becoming a secret store.** Per-request header forwarding
  through an explicit allow-list; the server forwards and forgets.
- **Ship output models can't be hijacked through.** Content-aware framing with nonce-guarded
  delimiters — indirect prompt injection mitigated at the source.

Every opinion is written down, versioned, and mapped to the spec section it narrows:
**[the Emperor Profile (P1)](./PROFILE.md)**. A server built on `emperor-mcp` is a fully
compliant MCP server — the profile narrows the spec's options, it never breaks them.

## Five-minute quickstart

```bash
cargo new my-mcp-server && cd my-mcp-server
cargo add emperor-mcp serde_json
cargo add tokio --features full
```

`src/main.rs` — a complete MCP server with one framed tool:

```rust
use emperor_mcp::{FrameKind, McpHandler, frame, init_tracing, serve_http};
use serde_json::{Value, json};
use std::sync::Arc;

struct MyServer;

impl McpHandler for MyServer {
    async fn handle(&self, request: Value) -> Option<Value> {
        let id = request.get("id")?.clone(); // notifications get no reply
        let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "my-mcp-server", "version": "0.1.0" }
            }),
            "tools/list" => json!({ "tools": [{
                "name": "echo",
                "description": "Echo text back as a framed computed result.",
                "inputSchema": { "type": "object",
                    "properties": { "text": { "type": "string" } }, "required": ["text"] }
            }]}),
            "tools/call" => {
                let text = request.pointer("/params/arguments/text")
                    .and_then(Value::as_str).unwrap_or("");
                json!({ "content": [{ "type": "text",
                    "text": frame(FrameKind::Computed, text) }], "isError": false })
            }
            _ => json!({}),
        };
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_tracing();
    serve_http("127.0.0.1:8017".parse().unwrap(), Arc::new(MyServer)).await
}
```

```bash
cargo run
# from another shell:
curl -s -X POST http://127.0.0.1:8017/ \
  -H 'Content-Type: application/json' -H 'Accept: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
# => {"id":1,"jsonrpc":"2.0","result":{"tools":[{"name":"echo", ...}]}}
```

Any MCP client speaking Streamable HTTP can now call your `echo` tool — and its output
arrives framed as data, not instructions.

## Why "emperor"?

Emperor penguins are the largest of all penguins — and the only ones that breed through the
Antarctic winter, the harshest environment on Earth. They don't survive it by improvising.
They survive it by protocol: the colony huddles in disciplined formation, every bird rotates
through the exposed edge, nobody freelances, and the whole system works precisely because the
rules are strict and everyone follows them.

That is exactly the stance this framework takes on running MCP servers in production. The
spec deliberately leaves many choices open — transports, session handling, credential passing.
Openness is right for a spec; in an enterprise deployment, every open choice is a place two
systems disagree at 3 a.m. `emperor-mcp` closes those choices with one opinionated, fully
spec-compliant profile — and like the huddle, none of it contradicts the wider colony.

The name is also a nod to this crate's origin: it hatched in the
[kowalski](https://github.com/yarenty/kowalski) rookery, where the penguins run in hordes.
Now it stands on its own ice.

## What's inside

- **Transport** — [`McpHandler`](src/transport.rs) + stateless Streamable HTTP + stdio (local/dev)
- **rmcp bootstrap** — [`serve`](src/serve.rs) at `/mcp` + `/health`, graceful shutdown
- **Output framing** — [`FrameKind`](src/framing.rs) / prompt-injection mitigation
- **Credential forwarding** — [`ForwardedHeaders`](src/headers.rs) for multi-tenant deployments

Two server styles, one set of rules:

```rust
// rmcp path: #[tool_router] handlers, credential forwarding wired in
use emperor_mcp::{init_tracing, serve, ServeOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    serve(ServeOptions::new("127.0.0.1:8080".parse()?), || MyServer::new()).await
}
```

For the hand-rolled `McpHandler` path, see the quickstart above or
[`examples/minimal_server.rs`](examples/minimal_server.rs). Production consumers:
[`kowalski-mcp-rookery`](https://github.com/yarenty/kowalski/tree/main/kowalski-mcp-rookery),
[`kowalski-mcp-datafusion`](https://github.com/yarenty/kowalski/tree/main/kowalski-mcp-datafusion).

## emperor-mcp vs plain rmcp

[`rmcp`](https://crates.io/crates/rmcp) is the official Rust MCP SDK, and `emperor-mcp` is
built on it — this is a layer, not an alternative.

| You need | Reach for |
|---|---|
| Full protocol breadth, custom transports, your own deployment opinions | `rmcp` directly |
| A production posture out of the box: stateless HTTP, credential forwarding, framed output, a written profile to hold servers to | `emperor-mcp` |
| A tiny hand-rolled server without SDK machinery | `emperor-mcp`'s `McpHandler` path |

`emperor-mcp` pre-makes the deployment decisions and documents them in
[PROFILE.md](./PROFILE.md); `rmcp` stays fully reachable underneath (the `serve` path takes
any rmcp `ServerHandler`), so you can drop down whenever you need the SDK's full surface.

## Authoring rules

- [`PROFILE.md`](./PROFILE.md) — **the Emperor Profile (P1)**: the versioned deployment profile, rule by rule, with spec mapping and enforcement
- [`MCP_REQUIREMENTS.md`](./MCP_REQUIREMENTS.md) — authoring checklist for servers built on this framework
- [`MANIFEST_SPEC.md`](./MANIFEST_SPEC.md) — the `manifest.yaml` each server ships

## Build

```bash
cargo test                                  # unit + HTTP integration tests
cargo build --examples && ./scripts/smoke.sh  # example server round-trip
```

**MSRV:** Rust 1.85 (edition 2024), declared via `rust-version` in `Cargo.toml`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md): run the checks, keep the docs in step, and sign off
your commits (DCO, `git commit -s`).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
emperor-mcp by you shall be dual licensed as above, without any additional terms or conditions.
