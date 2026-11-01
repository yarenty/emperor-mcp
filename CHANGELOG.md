# Changelog

All notable changes to emperor-mcp. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [semantic versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/yarenty/emperor-mcp/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/yarenty/emperor-mcp/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/yarenty/emperor-mcp/releases/tag/v0.1.0
