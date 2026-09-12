# Announcement drafts

> Drafts for the maintainer to publish manually (blog / LinkedIn). This file is the working
> copy; nothing here is auto-published.

---

## Blog post

### MCP servers keep dying on the way to production. So I wrote the rules down.

The Model Context Protocol solved the right problem: one protocol between AI applications
and the tools they call. The ecosystem exploded — thousands of servers, SDKs in every
language, support in every major client.

Then teams started deploying those servers in real infrastructure, and a pattern emerged.
The demo works; production doesn't. Not because MCP is broken — because it is *open*. The
spec deliberately leaves choices to implementers: two standard transports, optional sessions,
optional auth, no opinion on how tool output should be presented to a model. Openness is
exactly right for a specification. But in an enterprise deployment, every open choice is a
place where two systems disagree at 3 a.m.:

- A server that only speaks **stdio** is a subprocess, not a service. No lifecycle, no
  scaling, no network-level access control.
- **Optional sessions** become sticky-session requirements that break the moment a load
  balancer enters the picture.
- **Credential handling** is left to each server author — so every server invents its own,
  and some of them will store what they should have forwarded.
- **Tool output** goes to a language model verbatim. If a web page your tool fetched says
  "ignore your instructions", that text lands in the prompt unmarked. Indirect prompt
  injection is the dominant connector risk, and the spec is silent on it.

**[emperor-mcp](https://github.com/yarenty/emperor-mcp)** is my answer: a Rust server
framework that closes those choices, plus a document that makes the choices auditable —
**[the Emperor Profile](https://github.com/yarenty/emperor-mcp/blob/main/PROFILE.md)**, ten
numbered rules, each mapped to the exact spec section it narrows, adds to, or restates.
Never contradicts: a server built on emperor-mcp is a fully compliant MCP server and works
with every compliant client.

The name? Emperor penguins are the only penguins that breed through the Antarctic winter.
They don't survive it by improvising — they survive it by protocol: the huddle, the rotation,
every bird taking its turn on the exposed edge. Production is the Antarctic winter of
software. Discipline is the feature.

Concretely, the framework gives you:

- **Stateless Streamable HTTP** — no `Mcp-Session-Id`, every POST self-contained; regression
  tests assert no session header ever appears.
- **Credential forwarding, not storage** — an explicit header allow-list captured per
  request by middleware; the framework has no credential storage API at all.
- **Framed output** — every tool result wrapped in content-aware, nonce-guarded delimiters
  ("this is data, not instructions"), with delimiter forgery neutralized and tested.
- **A complete server in under 100 lines** — `cargo add emperor-mcp`, implement one trait,
  serve.

It's on [crates.io](https://crates.io/crates/emperor-mcp) as 0.1.0, MIT or Apache-2.0, MSRV 1.85, docs on
[docs.rs](https://docs.rs/emperor-mcp). The profile is honest about its own gaps (in-process
origin validation lands in 0.2, and a P2 profile is planned for the 2026-07-28 spec revision).
It hatched inside my [kowalski](https://github.com/yarenty/kowalski) agent framework, where
its two production servers came from; now it stands on its own ice.

If you're putting MCP servers behind a gateway, in front of a load balancer, or anywhere
credentials and untrusted content flow — have a look. And if you disagree with a rule, good:
the profile is versioned precisely so disagreement has somewhere to go.

---

## LinkedIn post

Every MCP server demo works. Then it meets production: a load balancer that hates sticky
sessions, credentials that need forwarding (not storing), and tool output that a model will
happily mistake for instructions.

The MCP spec leaves those choices open — rightly, it's a spec. But someone has to close them
per deployment, and "every team differently" is how 3 a.m. incidents happen.

So I wrote the choices down and shipped them as code: **emperor-mcp** — an enterprise MCP
server framework for Rust.

🐧 Stateless Streamable HTTP (no session affinity, ever)
🔐 Credential forwarding through an explicit allow-list — never stored
🛡️ Tool output framed as data, not instructions (prompt-injection mitigation, tested)
📜 The Emperor Profile: every rule versioned + mapped to the spec section it narrows —
never contradicts. Fully compliant MCP, discipline included.

Named after the only penguin that breeds through the Antarctic winter. It doesn't improvise;
it follows protocol. So should your servers.

crates.io: emperor-mcp · GitHub: github.com/yarenty/emperor-mcp · MIT or Apache-2.0

#rust #mcp #modelcontextprotocol #ai #llm #opensource
