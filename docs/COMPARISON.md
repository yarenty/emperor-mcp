# The emperor servers next to popular public MCP servers

Four MCP servers are built on emperor-mcp:
[emperor-folder](https://github.com/yarenty/emperor-folder),
[emperor-pdf](https://github.com/yarenty/emperor-pdf),
[emperor-mail](https://github.com/yarenty/emperor-mail) and
[emperor-vikunja](https://github.com/yarenty/emperor-vikunja). This page puts them next to the
most popular public servers for the same jobs, says what the emperor profile gives them, and
says where the others are better. Each server has its own, longer comparison (links at the end).

Figures for other projects (stars, last release, tool counts) were read from GitHub and the
projects' own READMEs and sources on **2026-11-01**. They change quickly; check the links.

## Background (plain English)

- **The spec moved on.** The current MCP revision is
  [2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28). It makes MCP stateless
  at the protocol level: no `initialize` handshake, no session ids, a new `server/discover` call.
  It deprecates "Roots" (the way clients told a server which folders it may touch) and retires
  the old HTTP+SSE transport. emperor-mcp was stateless from the start. On the stateless HTTP
  path it answers both the 2025-06-18 `initialize` and the 2026-07-28 `server/discover`, and
  tests both ([PROFILE.md](../PROFILE.md), P2 note).
- **Tool annotations** (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`)
  are labels a server puts on its tools so a client can ask "this will change things, allow?".
  The spec says clients must treat them as **untrusted hints**. They help the user interface;
  they are not a lock. The lock is what the server itself refuses to do.
- **EscapeRoute** (July 2025, CVE-2025-53109 and CVE-2025-53110). The official filesystem server
  checked "is this path inside the allowed folder?" by comparing text prefixes, so
  `/allowed_evil` passed for `/allowed`, and symlinks led out of the folder. It was fixed in
  2025.7.1 and hardened since, but the checks are still made on path names.
- **postmark-mcp** (September 2025). The first known malicious MCP server: an npm package that,
  after 15 clean versions, quietly BCC'd every e-mail it sent to the attacker. Installing an MCP
  server is installing code that sees your data.
- **The "lethal trifecta"** ([Simon Willison](https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/)):
  an assistant that reads private data, reads untrusted content and can send things out can be
  talked into leaking the data. File and mail servers sit right on top of it. The defence is to
  cut the outbound leg in the server, with switches and allow-lists, rather than hope the model
  behaves.

## Summary by domain

| Domain | emperor server | Most popular alternatives (stars, 2026-11-01) | In one line |
|---|---|---|---|
| Folder | emperor-folder: 7 tools, 4 when read-only | official `filesystem` (monorepo 91.7k, 14 tools), Desktop Commander (10.2k, 25 tools incl. a terminal), rust-mcp-filesystem (184, 24 tools) | The folder boundary is enforced by the operating system through cap-std, not by comparing path strings; read-only by default. |
| PDF | emperor-pdf: 6 tools, 4 when read-only | anymd (1.06k, 4 tools), pdf-mcp (165, 13 tools), markitdown-mcp (1 tool) | The only one that reads, searches, renders, merges/extracts **and** creates PDFs; flags hidden text. No OCR. |
| Mail | emperor-mail: 6 tools | mcp-email-server (374, 21 tools), google_workspace_mcp (3.46k), ms-365-mcp-server (1.10k) | Draft-only by default; sending needs a start-up switch and a recipient allow-list; credentials forwarded per request, never stored. |
| Vikunja | emperor-vikunja: 6 tools, 3 when read-only | democratize-technology/vikunja-mcp (114, 13 multiplexed tools), aimbitgmbh/vikunja-mcp (12, 52 tools) | The small one: no delete, `--read-only`, labels and assignees by name. |

## What the emperor profile gives every server

These come from emperor-mcp and the [Emperor Profile](../PROFILE.md), so they hold for all four
servers in the same way.

- **Stateless Streamable HTTP and stdio.** Every HTTP request stands on its own: no session id,
  no sticky load balancing, a restart loses nothing. stdio is there for desktop clients that
  start the server themselves. The official filesystem server, for one, is stdio-only.
- **Framed output.** Text that came from outside (a file, a PDF, an e-mail, a task description)
  reaches the model inside a frame with a per-call nonce that says "this is data, not
  instructions", and what kind of data it is (`Document`, `PrivateCommunication`, `Computed`). A
  document that contains a fake end-of-data marker cannot break out, because it cannot guess the
  nonce. It does not make prompt injection impossible; it gives the model a clear line to hold.
- **Forwarded credentials, never stored.** On a shared HTTP server each request brings its own
  credentials in headers (a Vikunja token, a mail password or an OAuth access token). The server
  uses them for that call and drops them. There is no token file, keyring or database to steal.
  When the upstream rejects them, the MCP response carries HTTP 401 or 403, so the client can ask
  the user to sign in again instead of reporting "the tool failed".
- **At most 8 tools.** Each server has 6 or 7, fewer when writing is off. A small tool list costs
  fewer tokens in every request, and small local models choose the right tool more often from a
  short list than from 50. No "mega-tools" that hide twenty operations behind a `command`
  argument, so a client can still tell a read from a write.
- **Write switches at start-up.** Writing is off unless the operator turns it on when starting
  the server (`--writable`, `--allow-send` with `--send-allow`), and the vikunja server can be
  started `--read-only`. Where a tool cannot be used it is not listed at all, except mail's `send`,
  which stays listed so its refusal can point the model to `draft`. Annotations are on every tool
  and are checked by a test in each repo, but the switches are the real lock.
- **`doctor`.** Each server has a `doctor` subcommand that checks its setup (folders, upstream
  reachability, login, listen address, the tool budget) without printing a secret, and exits 1
  when something is wrong, so scripts and container health checks can use it.
- **A single binary with provenance.** Each server is one Rust binary: no npm or PyPI install at
  start-up, no interpreter. Release binaries for Linux, macOS (two) and Windows carry a
  `SHA256SUMS` file and GitHub build provenance attestations
  (`gh attestation verify <file> --repo yarenty/<server>`), so you can check that a binary was
  built from the repository by its release workflow. All four are also on crates.io and ship a
  Dockerfile.

## Where others are better

The emperor servers are new and deliberately small. Pick something else when:

- **You need OCR.** emperor-pdf has none: a scanned page is rendered as an image for a
  vision-capable model to read. anymd and pdf-mcp run OCR locally.
- **You search across many long PDFs.** pdf-mcp's hybrid keyword and semantic search over a
  folder, with a cache, saves a lot of tokens; emperor-pdf searches text and regex only.
- **You want documents other than PDF.** anymd and markitdown-mcp read Word, PowerPoint, Excel,
  EPUB, HTML and more. (emperor-mail reads Word and spreadsheet attachments, but only inside mail.)
- **You need to organise or delete mail, forward it, or manage several accounts in one process.**
  mcp-email-server does all of that behind its own mutation switches (note that its default
  allows every class, including send and delete).
- **You want a full Google Workspace or Microsoft 365 integration** (calendar, drive, Graph API
  features, an OAuth flow run by the server). google_workspace_mcp and ms-365-mcp-server cover
  far more ground. emperor-mail speaks IMAP and SMTP and expects the client to bring the OAuth
  token.
- **You need several edits to a file per call, or a dry-run diff.** The official filesystem
  server's `edit_file` does both; emperor-folder's `edit` replaces one exact snippet per call.
- **You want the agent to run commands.** Desktop Commander gives a terminal. (That is also why
  its folder limits are "guardrails, not a sandbox", in its own words.)
- **You use most of Vikunja's API.** aimbitgmbh/vikunja-mcp covers projects, relations,
  buckets and more on Vikunja's API v2; emperor-vikunja covers tasks, labels, assignees,
  comments and listing projects.
- **You need a track record.** Every alternative named here has users; the emperor servers
  were released on 2026-11-01.

## Per-server comparisons

- [emperor-folder: folder access](https://github.com/yarenty/emperor-folder/blob/main/docs/COMPARISON.md)
- [emperor-pdf: PDF](https://github.com/yarenty/emperor-pdf/blob/main/docs/COMPARISON.md)
- [emperor-mail: mail](https://github.com/yarenty/emperor-mail/blob/main/docs/COMPARISON.md)
- [emperor-vikunja: Vikunja](https://github.com/yarenty/emperor-vikunja/blob/main/docs/COMPARISON.md)
