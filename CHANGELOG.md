# Changelog

All notable changes to emperor-mcp. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [semantic versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-11-01

> Credential rejection the client can act on, output limits, two new frame kinds, and kits for servers' own CLIs and tests.

### Added
- **Upstream credential rejection** - `credentials` module: a tool returns `credentials_rejected(&ctx, Rejection::Unauthorized, "upstream")` when the upstream answers 401/403, and the MCP HTTP response carries that status (401 with `WWW-Authenticate: Bearer error="invalid_token"`), so clients can ask the user to sign in again. The body stays a JSON-RPC error (code `CREDENTIALS_REJECTED` = -32040, `data.httpStatus`), which is all a stdio client sees. `serve` mounts `credential_rejection_middleware`; the `McpHandler` path answers `rejection_envelope` replies with the status too.
- **`FrameKind::Document`** (files and documents of unknown authorship) and **`FrameKind::PrivateCommunication`** (mail and messages written by third parties, addressed to the user, not the assistant).
- **`limits` module** - `Limits` with safe defaults and hard ceilings: characters returned (50 000, ceiling 200 000), results per call (20, ceiling 100), bytes per download or file (8 MiB, ceiling 64 MiB); `clamp_results`, `clamp_chars`, `bytes_allowed`, and `truncate_chars`, which cuts on a character boundary and says what was left out.
- **`doctor`** - `dir_readable` and `dir_writable` (creates and removes a probe file).
- **`cli` feature** - `cli::ServeArgs` (clap derive): `--bind`, `--stdio`, repeatable `--allowed-host` and `--allowed-origin`; `serve_options(default_bind)` builds `ServeOptions`. Off by default, so clap stays out of the default build.
- **`testkit` feature** - `testkit::check_tools` / `assert_tools` (rmcp `Tool` list) and `check_tools_json` / `assert_tools_json` (a `tools/list` answer): 1 to 8 tools, non-empty descriptions, explicit `readOnlyHint` (and `destructiveHint` on tools that write), object input schemas.
- Tests showing the stateless HTTP path answers both the 2025-06-18 `initialize` handshake and the 2026-07-28 `server/discover` call (PROFILE.md, P2 note).

### Changed
- `FrameKind` is `#[non_exhaustive]`: a `match` on it outside this crate needs a wildcard arm. Further kinds can then arrive in a minor release.

## [0.2.0] - 2026-11-01

> rmcp 3, in-process origin validation, and `doctor`.

### Added
- **`doctor` module** - `Report`, `Check` and generic checks (`port_free`, `env_set` that never prints a secret, `tcp_reachable`, `tool_budget`) for each server's `doctor` subcommand.
- **Tool budget** - servers expose 3 to 8 tools (`doctor::MAX_TOOLS`); `MCP_REQUIREMENTS.md` §11.
- **Host and Origin validation in-process** - `ServeOptions::with_allowed_hosts` and `with_allowed_origins` for deployments without a validating ingress (Emperor Profile E5). Off by default, as before.

### Changed
- **rmcp 3.8** (from 1.7). Public types from rmcp change with it: `framing` builds `ContentBlock` (was `Content`); handlers implement rmcp 3's `ServerHandler`.
- Minimum Rust version is 1.88 (rmcp 3's own minimum).

### Fixed
- `init_tracing` logs to stderr instead of stdout, so a server on the stdio transport keeps its protocol stream clean (Emperor Profile E9).

## [0.1.0] - 2026-08-27

### Added
- Stateless Streamable HTTP transport (`McpHandler`, `serve_http`, stdio for local use), rmcp bootstrap (`serve`), output framing (`FrameKind`), credential forwarding (`ForwardedHeaders`), Emperor Profile P1.

[Unreleased]: https://github.com/yarenty/emperor-mcp/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/yarenty/emperor-mcp/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/yarenty/emperor-mcp/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/yarenty/emperor-mcp/releases/tag/v0.1.0
