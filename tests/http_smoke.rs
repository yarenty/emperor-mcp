//! Fresh-clone integration smoke: a consumer using only the public API can build a
//! stateless Streamable HTTP MCP server that answers initialize / tools/list / tools/call
//! with framed output — over real HTTP.

use emperor_mcp::{FrameKind, McpHandler, frame, http_router};
use serde_json::{Value, json};
use std::sync::Arc;

struct SmokeServer;

impl McpHandler for SmokeServer {
    async fn handle(&self, request: Value) -> Option<Value> {
        let id = request.get("id")?.clone();
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "smoke", "version": "0.0.0" }
            }),
            "tools/list" => json!({ "tools": [{ "name": "echo" }] }),
            "tools/call" => json!({
                "content": [{ "type": "text", "text": frame(FrameKind::Computed, "hello") }],
                "isError": false
            }),
            _ => json!({}),
        };
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

async fn spawn() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, http_router(Arc::new(SmokeServer)))
            .await
            .unwrap();
    });
    format!("http://127.0.0.1:{}/", addr.port())
}

async fn call(url: &str, body: Value) -> Value {
    reqwest::Client::new()
        .post(url)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn initialize_then_tools_list_then_framed_call_round_trip() {
    let url = spawn().await;

    let init = call(
        &url,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
    )
    .await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert!(init["result"]["capabilities"]["tools"].is_object());

    let tools = call(
        &url,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await;
    assert_eq!(tools["result"]["tools"][0]["name"], "echo");

    let reply = call(
        &url,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "echo", "arguments": { "text": "hello" } }
        }),
    )
    .await;
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("BEGIN_DATA_"), "tool output must be framed");
    assert!(text.contains("hello"));
}
