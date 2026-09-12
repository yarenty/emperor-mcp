# Contributing to emperor-mcp

Thanks for helping. Small, focused pull requests are the easiest to review; open an issue first
for anything that touches a rule in [PROFILE.md](./PROFILE.md) (Emperor Profile P1).

## Before you open a pull request

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --examples && ./scripts/smoke.sh
```

Keep the docs in step with the code: `README.md`, `MCP_REQUIREMENTS.md`, `AGENTS.md`, and
`PROFILE.md` when a profile rule is affected. Every public item is documented
(`#![warn(missing_docs)]`).

## Developer Certificate of Origin

This project uses the [Developer Certificate of Origin](https://developercertificate.org/)
(DCO) instead of a contributor licence agreement. By adding a sign-off line to your commits
you certify that you wrote the change or otherwise have the right to submit it under the
project licence.

Sign off every commit:

```bash
git commit -s
```

which adds a line such as

```
Signed-off-by: Your Name <you@example.com>
```

Use a real name and a reachable email address.

## Licence

emperor-mcp is dual licensed under MIT OR Apache-2.0 (see [LICENSE-MIT](./LICENSE-MIT) and
[LICENSE-APACHE](./LICENSE-APACHE)). Unless you explicitly state otherwise, any contribution
you submit is licensed the same way, without any additional terms or conditions.
