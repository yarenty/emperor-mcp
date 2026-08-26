# MCP manifest spec

> Each first-party server ships **`manifest.yaml`** next to its crate.

## File location

```
<server-crate>/manifest.yaml
```

## Required fields

| Field | Purpose |
|-------|---------|
| `name` | Short id (matches crate suffix) |
| `type` | `server` |
| `about.title` | Operator-facing name |
| `about.description` | One-line summary |
| `meta.category` | UI grouping hint |
| `tools[]` | Advertised tool names + descriptions |
| `test_tool` | Cheap smoke call (`name` + `args`) |

## Host-application fields

| Field | Purpose |
|-------|---------|
| `endpoint` | Default Streamable HTTP URL (trailing `/` for `McpHandler` servers, `/mcp` for rmcp `serve`) |
| `topology` | `local-config` (env/`config.toml`) or `forward-headers` (multi-tenant; use `headers:` block) |
| `config.parameters` | JSON Schema for operator settings (paths, API keys) |
| `headers` | Optional per-request header templates when `topology: forward-headers` |

## Example (no credentials)

See [`kowalski-mcp-rookery/manifest.yaml`](https://github.com/yarenty/kowalski/blob/main/kowalski-mcp-rookery/manifest.yaml).

## Registration in a host application (example: kowalski)

Operators add servers in **`resources/config.toml`**:

```toml
[[mcp.servers]]
name = "rookery"
transport = "stdio"
command = ["cargo", "run", "-p", "kowalski-mcp-rookery", "--", "--transport", "stdio"]
```

HTTP example:

```toml
[[mcp.servers]]
name = "datafusion"
transport = "http"
url = "http://127.0.0.1:8080/"
```

Future work may import `manifest.yaml` into the Vue MCP panel automatically; until then manifests are the **source of truth** for tool lists and onboarding copy.

Do **not** rewrite third-party MCPs; these rules cover servers authored on this framework.
