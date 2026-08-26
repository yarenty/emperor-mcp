//! Minimal MCP server on the `emperor-mcp` framework: stateless Streamable HTTP,
//! one `echo` tool whose output is framed as data (prompt-injection mitigation).
//!
//! Run: `cargo run --example minimal_server` (port via `EMPEROR_PORT`, default 8017).

use emperor_mcp::{FrameKind, McpHandler, frame, init_tracing, serve_http};
use serde_json::{Value, json};
use std::sync::Arc;

struct MinimalServer;

impl McpHandler for MinimalServer {
    async fn handle(&self, request: Value) -> Option<Value> {
        // Notifications carry no `id` and must not be answered.
        let id = request.get("id")?.clone();
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "minimal-server",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
            "ping" => json!({}),
            "tools/list" => json!({
                "tools": [{
                    "name": "echo",
                    "description": "Echo the given text back as a framed computed result.",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "text": { "type": "string" } },
                        "required": ["text"]
                    }
                }]
            }),
            "tools/call" => {
                let text = request
                    .pointer("/params/arguments/text")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                json!({
                    "content": [{ "type": "text", "text": frame(FrameKind::Computed, text) }],
                    "isError": false
                })
            }
            other => {
                return Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("method not found: {other}") }
                }));
            }
        };

        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_tracing();
    let port: u16 = std::env::var("EMPEROR_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8017);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    eprintln!("minimal_server listening on http://{addr}/");
    serve_http(addr, Arc::new(MinimalServer)).await
}
