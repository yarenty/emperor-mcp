# The Emperor Profile — P1

**Profile version:** P1 · **Written against MCP spec revision:** [2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18)
· **Applies to:** `emperor-mcp` 0.1.x

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
- **Enforcement:** by code. [`serve.rs`](src/serve.rs) pins `STATEFUL_MODE = false` (rmcp
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
- **Enforcement:** by policy, and this is P1's **known code gap**: [`serve.rs`](src/serve.rs)
  currently disables rmcp's in-process host validation (`disable_allowed_hosts`) under the
  trusted-ingress assumption, and the examples bind `127.0.0.1`. A configuration option to
  enforce origin/host validation in-process (for deployments without an ingress) is planned
  for 0.2 — until then, running without a validating ingress is out of profile.

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
**MUST** be accurate for the content source (`ExternalWeb` / `TrustedReference` / `Computed`).

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

---

## Compliance table

| Rule | Spec relation (2025-06-18) | Enforcement |
|------|----------------------------|-------------|
| E1 transport choice | narrows: transports § (both standard) | design + policy |
| E2 stateless | narrows: transports § Session Management ("MAY assign") | code: `serve.rs`, `transport.rs` + test |
| E3 endpoints | restates single endpoint; adds `/health` | code (rmcp path) / policy (`McpHandler` path) |
| E4 wire behavior | restates: transports § Sending Messages | code: `transport.rs` + tests |
| E5 ingress & origin validation | restates Security Warning; narrows deployment shape | policy; **code gap tracked for 0.2** |
| E6 credential forwarding | adds (spec silent on upstream credentials) | code: `headers.rs` + test; logging rule = policy |
| E7 no auth shortcuts | adds | policy |
| E8 output framing | adds (spec silent on prompt injection) | code: `framing.rs` + tests; application = policy |
| E9 clean streams / tracing | restates stdio §; adds structured logging | code direction; tool-code discipline = policy |
| E10 error envelopes | restates JSON-RPC semantics | code: `transport.rs` + test |

## Profile versioning

- **P1** (this document) is written against spec revision **2025-06-18** — the revision the
  underlying SDK path implements (initialization-handshake protocol family, which the current
  spec documents for backward compatibility).
- The **current** spec revision is [2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28),
  which replaces the initialize handshake with per-request version declaration
  (`MCP-Protocol-Version` header / `_meta`) and a mandatory `server/discover` RPC. **P2** will
  cover that family (tracked with the framework's 0.2 milestone, together with the E5 origin
  validation option).
- A profile version bump is required whenever a rule is added, removed, or its spec mapping
  changes. Servers declare the profile they target as "Emperor Profile P1".
