# emperor-mcp

[![CI](https://github.com/yarenty/emperor-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/yarenty/emperor-mcp/actions/workflows/ci.yml)

**The enterprise MCP server framework for Rust — Streamable HTTP first, credential forwarding,
audited framing.**

## Why "emperor"?

Emperor penguins are the largest of all penguins — and the only ones that breed through the
Antarctic winter, the harshest environment on Earth. They don't survive it by improvising.
They survive it by protocol: the colony huddles in disciplined formation, every bird rotates
through the exposed edge, nobody freelances, and the whole system works precisely because the
rules are strict and everyone follows them.

That is exactly the stance this framework takes on running MCP servers in production. The
[Model Context Protocol](https://modelcontextprotocol.io) deliberately leaves many choices
open — transports, session handling, credential passing. Openness is right for a spec; in an
enterprise deployment, every open choice is a place two systems disagree at 3 a.m.
`emperor-mcp` closes those choices with one opinionated, fully spec-compliant profile:

- **Streamable HTTP in production.** Servers are network services with real lifecycles,
  observability, and access control. (stdio transport is provided for local development and
  desktop-style embedding — the profile keeps it out of production deployments.)
- **Credential forwarding, not credential storage.** Per-request credentials travel in headers
  under explicit rules; the framework never becomes a secret store.
- **Strict, audited framing.** One way to frame requests and responses, checked, with no
  silent tolerance for ambiguity — including prompt-injection mitigation on tool output.
- **Stateless by default.** No session affinity to break your load balancer.

Like the huddle, none of this contradicts the wider colony: a server built on `emperor-mcp`
speaks standard MCP and works with every compliant client. The profile narrows the spec's
options; it never breaks them. The full ruleset is **[the Emperor Profile (P1)](./PROFILE.md)**
— ten numbered rules, each mapped to the MCP spec section it narrows, adds to, or restates,
with the module that enforces it (or the policy that governs it) named explicitly.

The name is also a nod to this crate's origin: it hatched in the
[kowalski](https://github.com/yarenty/kowalski) rookery, where the penguins run in hordes.
Now it stands on its own ice.

## What's inside

- **Transport** — [`McpHandler`](src/transport.rs) + stateless Streamable HTTP + stdio (local/dev)
- **rmcp bootstrap** — [`serve`](src/serve.rs) at `/mcp` + `/health`
- **Output framing** — [`FrameKind`](src/framing.rs) / prompt-injection mitigation
- **Credential forwarding** — [`ForwardedHeaders`](src/headers.rs) for multi-tenant deployments

## Quick start (rmcp server)

```rust
use emperor_mcp::{init_tracing, serve, ServeOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let bind = "127.0.0.1:8080".parse()?;
    serve(ServeOptions::new(bind), || MyServer::new()).await
}
```

## Quick start (`McpHandler` server)

See [`examples/minimal_server.rs`](examples/minimal_server.rs) — a complete MCP server
(initialize / tools/list / tools/call with framed output) in under 100 lines:

```bash
cargo run --example minimal_server
# then, from another shell:
curl -s -X POST http://127.0.0.1:8017/ \
  -H 'Content-Type: application/json' -H 'Accept: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

Production consumers built on this framework:
[`kowalski-mcp-rookery`](https://github.com/yarenty/kowalski/tree/main/kowalski-mcp-rookery),
[`kowalski-mcp-datafusion`](https://github.com/yarenty/kowalski/tree/main/kowalski-mcp-datafusion).

## Authoring rules

- [`PROFILE.md`](./PROFILE.md) — **the Emperor Profile (P1)**: the versioned deployment profile, rule by rule, with spec mapping and enforcement
- [`MCP_REQUIREMENTS.md`](./MCP_REQUIREMENTS.md) — authoring checklist for servers built on this framework
- [`MANIFEST_SPEC.md`](./MANIFEST_SPEC.md) — the `manifest.yaml` each server ships

## Build

```bash
cargo test
```

**MSRV:** Rust 1.85 (edition 2024), declared via `rust-version` in `Cargo.toml`.

## License

MIT
