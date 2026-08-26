# emperor-mcp — AI agent notes

**Crate**: `emperor-mcp` · **Version**: 0.1.0 (pre-publish)

## Scope

Opinionated MCP server framework: stateless Streamable HTTP transport, content-aware output
framing (prompt-injection mitigation), per-request credential forwarding, rmcp bootstrap.
Hatched from the [kowalski](https://github.com/yarenty/kowalski) workspace
(`kowalski-mcp-base`); the servers there (`kowalski-mcp-rookery`, `kowalski-mcp-datafusion`)
are the reference consumers.

## Modules

| Module | Use |
|--------|-----|
| `transport` | `McpHandler`, `run_stdio` (local/dev), `serve_http`, `http_router` |
| `serve` | rmcp `ServerHandler` bootstrap (`/mcp`, `/health`) with credential forwarding |
| `framing` | `FrameKind`, `frame`, `structured_framed` |
| `headers` | `ForwardConfig`, `ForwardedHeaders`, middleware |

## Before you change code

1. Read [`MCP_REQUIREMENTS.md`](./MCP_REQUIREMENTS.md) and [`MANIFEST_SPEC.md`](./MANIFEST_SPEC.md).
2. `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test`
3. `cargo build --examples && ./scripts/smoke.sh` (example server round-trip)

## Hard rules

- **Stateless HTTP** — no mandatory `Mcp-Session-Id`.
- **Framing at source** — tool output is data, never instructions (`FrameKind`).
- **No auth shortcuts** — no dev-mode credential bypasses; credentials are forwarded, never stored.
- **Streamable HTTP in production** — stdio stays a local/dev transport.
- `#![warn(missing_docs)]` is on: every public item stays documented.
- Single logging facade: `tracing` (never add `log`).
- **MSRV 1.85** (edition 2024) — declared in `Cargo.toml`; bumping it is a semver-relevant decision.

## Documentation closure

Update this file, `README.md`, `MCP_REQUIREMENTS.md`, and downstream consumers' docs when
behavior changes. The versioned deployment profile (PROFILE.md) formalizes the hard rules.
