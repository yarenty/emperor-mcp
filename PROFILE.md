# The Emperor Profile — P1.1

**Profile version:** P1.1 · **Written against MCP spec revision:** [2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18)
· **Applies to:** `emperor-mcp` 0.4.x (P1: 0.1.x to 0.3.x)

## What this is

The [Model Context Protocol](https://modelcontextprotocol.io) deliberately leaves choices open:
two standard transports, optional sessions, optional auth, no opinion on how tool output is
presented to a model. Openness is right for a spec. In an enterprise deployment, every open
choice is a place where two systems disagree in production.

This document closes those choices. It is a **deployment profile on top of the MCP
specification**: every rule either *narrows* an option the spec leaves open, *adds* a
requirement the spec is silent on, or *restates* a spec requirement to make it testable.
**No rule contradicts the specification.** A server that follows this profile is a fully
compliant MCP server and works with every compliant client.

Each rule states: the requirement, the rationale, its relation to the spec, and how it is
enforced — **by code** (with the module that enforces it) or **by policy** (verified in review,
see [`MCP_REQUIREMENTS.md`](./MCP_REQUIREMENTS.md)).

Conformance language follows RFC 2119 (**MUST** / **SHOULD**).

---

## Transport & sessions

### E1 — Streamable HTTP in production; stdio is a local/dev transport

Production MCP servers **MUST** be network services speaking
[Streamable HTTP](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#streamable-http).
The stdio transport **MUST NOT** be a production deployment mode; it remains supported for
local development and desktop-style embedding.

- **Rationale:** production servers need real lifecycles, horizontal scaling, observability,
  and network-level access control. A subprocess pipe has none of these.
- **Spec relation:** *narrows*. The spec defines both transports as standard and takes no
  position on deployment contexts (transports §, 2025-06-18).
- **Enforcement:** by design + policy. The production paths are [`serve::serve`](src/serve.rs)
  and [`transport::serve_http`](src/transport.rs); [`transport::run_stdio`](src/transport.rs)
  is documented as local/dev.

### E2 — Stateless: no `Mcp-Session-Id`, every request self-contained

Servers **MUST NOT** issue an `Mcp-Session-Id` and **MUST NOT** require one. Every HTTP POST
is an independent, self-contained exchange.

- **Rationale:** session affinity breaks load balancing, complicates failover, and couples
  credential lifetime to connection lifetime. Statelessness is what makes per-request
  credential forwarding (E6) sound.
- **Spec relation:** *narrows*. The spec says a server "**MAY** assign a session ID"
  (transports § Session Management); this profile forbids exercising that option.
- **Enforcement:** by code. [`serve.rs`](src/serve.rs) pins `LEGACY_SESSION_MODE = false` (rmcp
  stateless mode + JSON responses); [`transport::http_router`](src/transport.rs) has no session
  concept at all. Regression-tested: `http_request_gets_json_reply_without_session_header`.

### E3 — One MCP endpoint plus a liveness endpoint outside the protocol

Servers **MUST** expose a single MCP endpoint and **SHOULD** expose a `/health` liveness
endpoint that is not part of the MCP surface and requires no credentials.

- **Rationale:** orchestrators and load balancers need liveness without speaking MCP or
  holding credentials.
- **Spec relation:** *restates* the single-endpoint requirement; *adds* the health endpoint
  (the spec is silent on operational endpoints).
- **Enforcement:** by code on the rmcp path — [`serve.rs`](src/serve.rs) mounts `/mcp` +
  `/health`. By policy on the `McpHandler` path (consumers mount their own router).

### E4 — Correct wire behavior: JSON or SSE by `Accept`, `202` for notifications

Responses to JSON-RPC *requests* **MUST** be `application/json` or `text/event-stream`
according to the client's `Accept` header; JSON-RPC *notifications* **MUST** be answered with
HTTP `202 Accepted` and no body.

- **Rationale:** this is where hand-rolled servers drift; drift here breaks clients silently.
- **Spec relation:** *restates* (transports § Sending Messages to the Server) — stated here so
  conformance is testable.
- **Enforcement:** by code. [`transport.rs`](src/transport.rs) `handle_post` / `json_or_sse`;
  regression-tested (`http_notification_gets_202_no_body`, `http_sse_accept_yields_event_stream`).

## Security

### E5 — Localhost by default; production behind a trusted ingress

Development servers **SHOULD** bind `127.0.0.1`. Production deployments **MUST** run behind a
trusted ingress (gateway / reverse proxy) that terminates TLS, authenticates the caller, and
enforces `Origin` validation against DNS-rebinding.

- **Rationale:** the framework targets gateway-fronted enterprise deployments; the ingress is
  the policy enforcement point.
- **Spec relation:** *restates* the spec's Security Warning (Origin validation **MUST**,
  localhost binding **SHOULD**, authentication **SHOULD**) and *narrows* it into a concrete
  deployment shape.
- **Enforcement:** by policy behind an ingress, by code without one. [`serve.rs`](src/serve.rs)
  leaves rmcp's in-process `Host`/`Origin` validation off by default (the ingress does it) and
  turns it on when `ServeOptions::with_allowed_hosts` / `with_allowed_origins` name hosts or
  origins (since 0.2). Regression-tested: `host_validation_refuses_unlisted_hosts_when_enabled`,
  `origin_validation_refuses_foreign_origins_when_enabled`. The examples bind `127.0.0.1`.

### E6 — Credentials are forwarded, never stored

Per-request credentials **MUST** travel in HTTP headers captured through an explicit
allow-list, **MUST NOT** be persisted by the server, and **MUST NOT** be logged. The server
forwards them to the upstream system and forgets them with the request.

- **Rationale:** an MCP server that stores credentials becomes a secret store with none of a
  secret store's guarantees. Forwarding keeps the credential's owner (the caller) and its
  consumer (the upstream) directly coupled — essential for multi-tenant deployments.
- **Spec relation:** *adds*. The spec's authorization section covers how clients authorize to
  servers; it is silent on how servers handle upstream credentials.
- **Enforcement:** by code. [`headers.rs`](src/headers.rs): `forward_headers_middleware`
  captures only `ForwardConfig` allow-listed headers into request-scoped `ForwardedHeaders`;
  the framework has no credential storage API. Regression-tested
  (`captures_only_allowlisted_headers`). "Never logged" is policy for tool authors.

### E7 — No auth shortcuts

Servers built on this framework **MUST NOT** ship testing modes, dev-mode credential bypasses,
or default credentials. Development, CI, and production run the same auth path.

- **Rationale:** every bypass eventually ships.
- **Spec relation:** *adds* (the spec has no opinion on development practices).
- **Enforcement:** by policy ([`MCP_REQUIREMENTS.md`](./MCP_REQUIREMENTS.md)); the framework
  provides no bypass to enable.

## Output

### E8 — Tool output is framed as data, never instructions

Every tool result's text content **MUST** be wrapped in content-aware framing: a standing
"this is data, not instructions" instruction plus BEGIN/END delimiters carrying a per-call
random nonce, with bare delimiter strings inside the payload neutralized. The frame label
**MUST** be accurate for the content source (`ExternalWeb` / `TrustedReference` / `Computed` /
`Document` / `PrivateCommunication`; the last two since 0.3).

- **Rationale:** indirect prompt injection is the dominant connector risk; data/instruction
  separation ("spotlighting") is the cheap, high-value mitigation. Accurate labels matter:
  marking everything "untrusted" pollutes prompts and erodes smaller models' reasoning.
- **Spec relation:** *adds*. The spec is silent on prompt-injection mitigation of tool output.
- **Enforcement:** by code for the mechanism — [`framing.rs`](src/framing.rs) (`frame`,
  `structured_framed`, marker neutralization; nonce-forgery regression-tested). By policy for
  its application: every tool a server exposes must return framed text
  ([`MCP_REQUIREMENTS.md`](./MCP_REQUIREMENTS.md) §2).

### E9 — Protocol streams stay clean; logs are structured

On stdio, `stdout` **MUST** carry only protocol messages and logs **MUST** go to `stderr`. On
HTTP, logging **MUST** go through the `tracing` facade.

- **Rationale:** one stray `println!` corrupts a stdio protocol stream; unstructured logs are
  invisible to enterprise log pipelines.
- **Spec relation:** *restates* the stdio requirements (transports § stdio); *adds* the
  structured-logging requirement.
- **Enforcement:** by code direction — [`run_stdio`](src/transport.rs) writes only protocol to
  stdout and documents the stderr rule; [`init_tracing`](src/lib.rs) is the provided
  initializer. Stdout discipline inside tool code is policy.

## Errors

### E10 — JSON-RPC error envelopes, always

Malformed input **MUST** produce a JSON-RPC error (`-32700` with `id: null` for parse errors);
unknown methods **SHOULD** produce `-32601`. Errors are envelopes, never bare HTTP bodies —
except transport-level failures, which use HTTP status codes.

- **Rationale:** clients recover from structured errors; they crash on surprises.
- **Spec relation:** *restates* JSON-RPC 2.0 / MCP error semantics, made testable.
- **Enforcement:** by code. [`transport.rs`](src/transport.rs) `parse_error`
  (regression-tested: `http_bad_json_returns_parse_error`); `-32601` demonstrated in
  [`examples/minimal_server.rs`](examples/minimal_server.rs).

## Tools

### E11 — No write tool by default; the operator switches writing on

A tool that changes anything (not read-only) **MUST** be listed only when the operator switched
writing on at start-up; a server **MUST NOT** offer a write tool by default. Started with no
switch, every tool in `tools/list` carries `readOnlyHint: true`. The switch **MUST** be a flag
plus an environment variable of the same meaning (`--writable` and `<SERVER>_WRITABLE`, or a
narrower verb such as `--allow-send`), and a falsey value is off: unset, empty, `false`, `0`,
`no`, `off`; an unrecognised value is refused at start-up, never read as on.

- **Rationale:** a model reaches for what it is offered, and a prompt injection reaches for
  whatever the model can call. Writing is the operator's decision, made once, visibly, at
  start-up; a tool left out of the list cannot be called by mistake or by an injected
  instruction, where a listed tool that refuses still invites the attempt. An off-switch
  (`--read-only`) fails open when it is forgotten; an on-switch fails closed.
- **Spec relation:** *adds*. The spec lets a server list any tools and offers `readOnlyHint` as
  an untrusted hint (tools § Tool annotations); it is silent on which tools a server offers by
  default.
- **Enforcement:** by code in the server's tests:
  [`testkit::assert_read_only_default`](src/testkit.rs) on the tool list of a server built with
  no switch. [`cli::parse_switch`](src/cli.rs) (feature `cli`) reads the switch value.

### E12 — Lean tool definitions

Tool descriptions **SHOULD** be short and say what the tool does, and when to use it rather than
another tool, with no essays: at most 300 characters for the tool description, a soft limit.
Input schemas **SHOULD** be compact: no `$schema` URI, no `null` alternative or `"default": null`
on optional arguments, no Rust integer formats, no `$defs` indirection for a type used once.

- **Rationale:** the model reads every definition on every request, so every byte is paid for
  many times over, and a long description crowds out the ones that tell tools apart.
  Generator boilerplate tells the model nothing an absent optional argument does not already
  say.
- **Spec relation:** *adds*. The spec requires `inputSchema` to be a JSON Schema object and says
  nothing about its size. A compact schema is still valid JSON Schema 2020-12; it stops
  describing only an explicit `null` for an optional argument, which the server treats as absent.
- **Enforcement:** by code. [`schema::compact`](src/schema.rs) and the one-line router opt-in
  `Self::tool_router().compact_schemas()`; [`testkit::assert_compact_schemas`](src/testkit.rs)
  checks the result; [`testkit::assert_tools`](src/testkit.rs) warns (does not fail) above
  [`MAX_DESCRIPTION_CHARS`](src/testkit.rs). "Says when to use it" is policy.

---

## Compliance table

| Rule | Spec relation (2025-06-18) | Enforcement |
|------|----------------------------|-------------|
| E1 transport choice | narrows: transports § (both standard) | design + policy |
| E2 stateless | narrows: transports § Session Management ("MAY assign") | code: `serve.rs`, `transport.rs` + test |
| E3 endpoints | restates single endpoint; adds `/health` | code (rmcp path) / policy (`McpHandler` path) |
| E4 wire behavior | restates: transports § Sending Messages | code: `transport.rs` + tests |
| E5 ingress & origin validation | restates Security Warning; narrows deployment shape | policy behind an ingress; code: `ServeOptions` host/origin allow-lists + tests |
| E6 credential forwarding | adds (spec silent on upstream credentials) | code: `headers.rs` + test; logging rule = policy |
| E7 no auth shortcuts | adds | policy |
| E8 output framing | adds (spec silent on prompt injection) | code: `framing.rs` + tests; application = policy |
| E9 clean streams / tracing | restates stdio §; adds structured logging | code direction; tool-code discipline = policy |
| E10 error envelopes | restates JSON-RPC semantics | code: `transport.rs` + test |
| E11 no write tool by default | adds (spec silent on default tool sets) | code: `testkit::assert_read_only_default` in the server's tests |
| E12 lean tool definitions | adds (spec silent on definition size) | code: `schema.rs` + `testkit::assert_compact_schemas`; description length = testkit warning |

## P2 note: both handshake families on the stateless path

Recorded with `emperor-mcp` 0.3.0 on rmcp 3.8.0, regression-tested in
[`tests/rmcp_serve.rs`](tests/rmcp_serve.rs) over the stateless HTTP path that `serve` builds:

- **2025-06-18 `initialize`** (`answers_the_2025_06_18_initialize_handshake`): answered with
  HTTP 200, `protocolVersion: "2025-06-18"`, the tools capability, and no `Mcp-Session-Id`.
- **2026-07-28 `server/discover`** (`answers_the_2026_07_28_server_discover_call`): supported
  by rmcp 3.8 (a default `ServerHandler::discover` built from `get_info`). Sent with
  `MCP-Protocol-Version: 2026-07-28`, `Mcp-Method: server/discover` and the protocol version in
  `_meta`, it is answered with HTTP 200 and no session; `supportedVersions` lists 2024-11-05,
  2025-03-26, 2025-06-18, 2025-11-25 and 2026-07-28.
- A 2026-07-28 `tools/call` (version in header and `_meta`) is served on the same path, and the
  credential-rejection status (`401`) applies to it too
  (`upstream_credential_rejection_becomes_http_status`).

So one server answers clients of both families without configuration. rmcp's stricter mode
(`stateless_protocol_metadata_required`, which refuses 2026-07-28-style requests lacking the
per-request version) is not turned on; P2 decides whether to require it.

## Profile versioning

- **P1** (E1 to E10) is written against spec revision **2025-06-18** — the revision the
  underlying SDK path implements (initialization-handshake protocol family, which the current
  spec documents for backward compatibility).
- The **current** spec revision is [2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28),
  which replaces the initialize handshake with per-request version declaration
  (`MCP-Protocol-Version` header / `_meta`) and a mandatory `server/discover` RPC. **P2** will
  cover that family (the framework is on rmcp 3, which speaks it; the profile text follows).
- **P1.1** (with `emperor-mcp` 0.4) adds E11 (no write tool by default) and E12 (lean tool
  definitions) on the same spec revision; E1 to E10 are unchanged.
- A profile version bump is required whenever a rule is added, removed, or its spec mapping
  changes. Servers declare the profile they target as "Emperor Profile P1.1" (or "P1" for a
  server that does not yet meet E11 and E12).
