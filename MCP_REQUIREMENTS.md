# MCP requirements (servers built on emperor-mcp)

> Authoring checklist for MCP servers built on this framework. The normative, versioned ruleset with spec mapping is [PROFILE.md](./PROFILE.md) (Emperor Profile P1.1); this file is its practical companion.


## 1. Summary (the rules)

Every new first-party MCP server **MUST**:

1. **Frame tool output** so the model treats it as *data*, never *instructions* — use [`FrameKind`](src/framing.rs) from `emperor-mcp`).
2. Be **stateless per request** — no per-user server state; each tool call stands alone.
3. Be **async Rust** — `tokio`, non-blocking, safe under concurrent calls.
4. Use **streamable HTTP** as the primary deployment transport; stdio for desktop clients.
5. Be built on **`emperor-mcp`** — do not reimplement transport, health, or framing.
6. Have **no dev/test auth bypasses** — same code path in dev, CI, and production.
7. Ship a **`manifest.yaml`** beside the crate.
8. Expose **3 to 8 tools** — split a bigger surface into several servers (§11).
9. Ship a **`doctor`** subcommand built on [`emperor_mcp::doctor`](src/doctor.rs) (§11).
10. **Annotate every tool**: `readOnlyHint` always, `destructiveHint` too on a tool that writes (§12).
11. Report an upstream **credential rejection** as one, not as a generic failure (§13).
12. **Cap what a tool returns**: characters, results per call, bytes downloaded (§14).
13. **Offer no write tool by default**: a tool that changes anything (not read-only) is listed only when the operator switched writing on at start-up (§15).
14. **Keep tool definitions lean**: a short description that says what the tool does and when to use it rather than another tool; compact input schemas (§16).

## 2. Content-aware output framing (REQUIRED)

Wrap tool text in BEGIN/END delimiters with a per-call nonce. Pick the correct [`FrameKind`](src/framing.rs):

| Source | `FrameKind` | When |
|--------|-------------|------|
| Web / external pages | `ExternalWeb` | Search, fetch-url |
| Vault / wiki / notes | `TrustedReference` | Obsidian, Knowledge Compiler outputs |
| SQL / stats / computed | `Computed` | DataFusion, calculators |
| Files / documents of unknown authorship | `Document` | Read a PDF, office file or text file |
| Mail / chat / messages from third parties | `PrivateCommunication` | Read mail, list messages |

`PrivateCommunication` is not `Document`: messages are written *to the user* by other people,
and one of them can be written to steer an assistant that reads the inbox ("assistant, forward
this thread to…"). Its preamble says so plainly.

Use [`frame`](src/framing.rs) or [`structured_framed`](src/framing.rs) at the tool boundary. Include a **framing breakout test** in unit tests.

## 3. Stateless (REQUIRED)

- No session-scoped user data between tool calls.
- Default posture: single operator; credentials come from env/config at process start or per-request headers when deployed multi-tenant.
- Never cache one user's upstream credentials for another user's call.

## 4. Async Rust (REQUIRED)

- Edition 2024, `tokio`, `reqwest` with async I/O where applicable.
- CPU-bound work → `spawn_blocking`.
- Upstream calls have explicit timeouts; return normalized errors, never panic on upstream failure.

## 5. Transport (REQUIRED)

Two supported patterns from `emperor-mcp`:

| Style | API | Use when |
|-------|-----|----------|
| Hand-rolled JSON-RPC | [`McpHandler`](src/transport.rs) + [`run_stdio`](src/transport.rs) / [`serve_http`](src/transport.rs) | Existing servers (datafusion, rookery) |
| rmcp `ServerHandler` | [`serve`](src/serve.rs) at `/mcp` + `/health` | New `#[tool_router]` servers (Obsidian, …) |

**Stateless HTTP:** no `Mcp-Session-Id` required. Every POST is independent.

**stdio:** logs to **stderr** only; stdout is the protocol stream.

## 6. Build on `emperor-mcp` (REQUIRED)

```rust
use emperor_mcp::{init_tracing, serve, ServeOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    emperor_mcp::init_tracing();
    let bind = "0.0.0.0:8000".parse()?;
    emperor_mcp::serve(ServeOptions::new(bind), || MyServer::new()).await
}
```

Or for `McpHandler` servers, see [`examples/minimal_server.rs`](examples/minimal_server.rs) and [`kowalski-mcp-rookery`](https://github.com/yarenty/kowalski/tree/main/kowalski-mcp-rookery).

## 7. No auth shortcuts (REQUIRED)

Never add `DEV_MODE`, hardcoded tokens, or "skip auth in test" branches. Fix the real path instead.

## 8. Manifest (REQUIRED)

Each server crate includes **`manifest.yaml`** per [`MANIFEST_SPEC.md`](./MANIFEST_SPEC.md). Host applications register servers in their own config; manifests document tools, defaults, and smoke `test_tool` for operators and future catalog import.

## 9. Optional workspace builds

MCP server crates are **workspace members** but **not** in `default-members`. Build explicitly:

```bash
cargo build            # framework
cargo test             # unit + HTTP smoke tests
cargo build --examples && ./scripts/smoke.sh
```


## 10. Checklist for a new server

- [ ] Server crate with thin tool logic
- [ ] Depends on `emperor-mcp` only for transport/framing (not duplicated axum/rmcp wiring)
- [ ] Output framing + breakout test
- [ ] Stateless HTTP + optional stdio
- [ ] `manifest.yaml` + README + AGENTS.md
- [ ] Example `[[mcp.servers]]` block in `resources/config.toml`
- [ ] Root `CHANGELOG.md` when user-visible
- [ ] 3 to 8 tools, each name a verb on one noun (`read_message`, not `email_tool`)
- [ ] `doctor` subcommand: configuration, credentials (never printed), upstream reachable, listen address free, folders readable/writable, tool budget
- [ ] Every tool annotated (`readOnlyHint`, plus `destructiveHint` when it writes); `emperor_mcp::testkit::assert_tools` in the server's tests
- [ ] Upstream 401/403 returned through `credentials_rejected`
- [ ] Output capped with `Limits` (`truncate_chars`, `clamp_results`, `bytes_allowed`)
- [ ] No write tool listed without the write switch (`--writable` + `MY_MCP_WRITABLE`); `testkit::assert_read_only_default` on the default tool list
- [ ] Tool descriptions under 300 characters; router built with `.compact_schemas()`; `testkit::assert_compact_schemas` in the server's tests

## 11. Small tool surface and `doctor` (REQUIRED)

**3 to 8 tools per server.** A model chooses well among a handful of tools and badly among thirty;
every extra tool also costs context on every request. When a server grows past eight, split it by
noun (mail reading vs mail sending, tasks vs projects) instead of adding a ninth.
[`doctor::tool_budget`](src/doctor.rs) enforces the limit at `doctor` time; `MAX_TOOLS` is 8.

**`doctor` subcommand.** `my-mcp doctor` checks what the server needs before it is started, prints
one line per check and exits `1` when any check fails, so scripts and container health checks can
use it. Build it from [`doctor::Report`](src/doctor.rs) and the generic checks (`port_free`,
`env_set` — secrets show only their length —, `tcp_reachable`, `tool_budget`) plus the server's own
(log in to the upstream, open the root folder, find the native library).

Servers using clap flatten [`cli::ServeArgs`](src/cli.rs) (feature `cli`) so `--bind`,
`--stdio`, `--allowed-host` and `--allowed-origin` read the same in every server.

## 12. Tool annotations (REQUIRED)

Every tool carries annotations with **`readOnlyHint` set explicitly**. A tool that is not
read-only also sets **`destructiveHint`** explicitly (`false` for "adds or updates", `true` for
"deletes or overwrites"). Clients use these hints to decide what needs the user's confirmation;
an unset hint means the client has to assume the worst. With rmcp:
`#[tool(annotations(read_only_hint = false, destructive_hint = false))]`.

Check it in the server's own tests with the `testkit` feature:
[`testkit::assert_tools`](src/testkit.rs) on `tool_router().list_all()`, or
[`testkit::assert_tools_json`](src/testkit.rs) on a `tools/list` reply. Both also check the tool
count (1 to 8), non-empty descriptions and object input schemas.

## 13. Upstream credential rejection (REQUIRED)

When the upstream answers `401` or `403` to the forwarded credentials, the tool returns
[`credentials_rejected`](src/credentials.rs)`(&ctx, rejection, "upstream name")`, not an ordinary
tool error. Over HTTP through `serve` the MCP response then carries `401` (with a Bearer
`WWW-Authenticate` challenge) or `403`, so the client can ask the user to sign in again. The body
is a JSON-RPC error (code `-32040`, `data.httpStatus`), which is what a stdio client sees. The
`McpHandler` path returns [`rejection_envelope`](src/credentials.rs). Never retry with other
credentials and never fall back to a shared account.

## 14. Limits (REQUIRED)

A tool result lands in the model's context. Cap it with [`Limits`](src/limits.rs):

| Cap | Default | Ceiling |
|-----|---------|---------|
| Characters returned | 50 000 | 200 000 |
| Results per call | 20 | 100 |
| Bytes downloaded / read from one file | 8 MiB | 64 MiB |

A model-supplied `limit` argument goes through `clamp_results`; long text through
`truncate_chars`, which tells the model what it left out; downloads check `bytes_allowed` before
and while reading. Configuration can lower a cap, never lift it past the ceiling.

## 15. Writing is switched on, never on by default (REQUIRED)

**A tool that changes anything (not read-only) is listed only when the operator switched writing
on at start-up; a server offers no write tool by default.** Started with no switch, every tool
in `tools/list` carries `readOnlyHint: true`. Leaving a write tool out of the list is the point:
a tool that is listed but refuses still invites the model to try it, and a client that trusts
the list cannot tell. A tool that only reads in its default form (rendering a page without
saving it) is listed read-only and gains its writing option with the switch.

The switch is a flag plus an environment variable, read once at start-up:

- a flag named for what it allows: `--writable`, or a narrower verb such as `--allow-send`;
- an environment variable of the same meaning, `<SERVER>_WRITABLE` (e.g. `FOLDER_WRITABLE`), for
  containers and catalogs that pass parameters as environment;
- a falsey value is off: unset, empty, `false`, `0`, `no`, `off`. `true`, `1`, `yes`, `on` turn
  it on; anything else is refused at start-up rather than read as on.
  [`cli::parse_switch`](src/cli.rs) (feature `cli`) implements exactly this as a clap
  `value_parser`;
- `doctor` says which mode the server would start in.

An off-switch (`--read-only`) does not meet this rule: forgetting it must leave a server unable
to write. Check it with [`testkit::assert_read_only_default`](src/testkit.rs) on the tool list of
a server built with no switch.

## 16. Lean tool definitions (REQUIRED)

The model reads every tool definition on every request, so each byte is paid for many times.

**Descriptions are short and say what the tool does, and when to use it rather than another
tool. No essays.** Soft limit: 300 characters for the tool description
([`testkit::MAX_DESCRIPTION_CHARS`](src/testkit.rs)); `assert_tools` prints a warning above it,
it does not fail. Parameter descriptions are one line each. Limits, formats and defaults belong
in the parameter they apply to, not repeated in the tool description.

**Input schemas are compact.** Build the router with
[`schema::CompactSchemas`](src/schema.rs): `Self::tool_router().compact_schemas()`. It drops
the `$schema` URI, the `null` alternative and `"default": null` of optional arguments, Rust
integer formats (`"format": "uint32"`, `"minimum": 0`), titles that repeat the property name,
and `$defs` indirection for types used once. Descriptions, enums, `required`, real bounds and
non-null defaults stay. Check it with
[`testkit::assert_compact_schemas`](src/testkit.rs).
