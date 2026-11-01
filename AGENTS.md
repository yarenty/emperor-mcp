# emperor-mcp — AI agent notes

**Crate**: `emperor-mcp` · **Version**: 0.4.0 (0.3.0 on crates.io) · **Licence**: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`; keep `Cargo.toml`, the README badge and the licence section in agreement; contributions are signed off, see `CONTRIBUTING.md`)

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
| `framing` | `FrameKind` (`ExternalWeb`, `TrustedReference`, `Computed`, `Document`, `PrivateCommunication`; non-exhaustive), `frame`, `structured_framed` |
| `headers` | `ForwardConfig`, `ForwardedHeaders`, middleware |
| `credentials` | `Rejection`, `credentials_rejected` (upstream 401/403 becomes the MCP HTTP status), `rejection_envelope` (`McpHandler` path), `credential_rejection_middleware`, `CREDENTIALS_REJECTED` |
| `limits` | `Limits` (chars 50k/200k, results 20/100, bytes 8/64 MiB: default/ceiling), `truncate_chars` |
| `schema` | `compact`, `compact_output`, `compact_tool`, `compact_tools`, `compact_list`, `compact_router`, `CompactSchemas` (`Self::tool_router().compact_schemas()`): lean `tools/list` schemas |
| `doctor` | `Report`, `Check`, `port_free`, `env_set`, `tcp_reachable`, `dir_readable`, `dir_writable`, `tool_budget` (`MAX_TOOLS` = 8) |
| `cli` (feature `cli`) | `ServeArgs`: `--bind`, `--stdio`, `--allowed-host`, `--allowed-origin`, `serve_options(default_bind)`; `parse_switch` for start-up switches (`--writable` + env) |
| `testkit` (feature `testkit`) | `check_tools`, `check_tools_json`, `assert_tools`, `assert_tools_json`: tool-list conformance; `check_read_only_default` / `assert_read_only_default` (no write tool without the switch), `check_compact_schemas` / `assert_compact_schemas`, `warnings` (descriptions over `MAX_DESCRIPTION_CHARS` = 300), each with a `_json` form |

## Before you change code

1. Read [`MCP_REQUIREMENTS.md`](./MCP_REQUIREMENTS.md) and [`MANIFEST_SPEC.md`](./MANIFEST_SPEC.md).
2. `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features`
3. `cargo build --examples && ./scripts/smoke.sh` (example server round-trip)

## Hard rules

- **Stateless HTTP** — no mandatory `Mcp-Session-Id`.
- **Framing at source** — tool output is data, never instructions (`FrameKind`).
- **No auth shortcuts** — no dev-mode credential bypasses; credentials are forwarded, never stored.
- **Streamable HTTP in production** — stdio stays a local/dev transport.
- `#![warn(missing_docs)]` is on: every public item stays documented.
- Single logging facade: `tracing` (never add `log`).
- Default features stay lean: `clap` only behind `cli`, conformance helpers only behind `testkit`.
  A TLS stack is not a dependency (the `doctor` TLS probe was left out for that reason).
- **rmcp 3.x**; **MSRV 1.88** (edition 2024, rmcp 3's own minimum) — declared in `Cargo.toml`; bumping it is a semver-relevant decision.

## Documentation closure

Update this file, `README.md`, `MCP_REQUIREMENTS.md`, and downstream consumers' docs when
behavior changes. The hard rules are formalized in [`PROFILE.md`](./PROFILE.md) (Emperor
Profile P1.1, mapped to MCP spec 2025-06-18; E11 and E12 added with 0.4): behavior changes that touch a profile rule
require a profile version review. The E5 in-process origin validation option shipped in 0.2;
the P2 profile for spec revision 2026-07-28 is pending (see the P2 note in `PROFILE.md`).
