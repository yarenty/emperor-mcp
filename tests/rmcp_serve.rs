//! The rmcp path end to end: a `#[tool_router]` server mounted by [`emperor_mcp::serve`]
//! answers `tools/list` and `tools/call` over stateless Streamable HTTP, frames its output,
//! and sees the forwarded `Authorization` header inside the tool.

use emperor_mcp::{ForwardedHeaders, FrameKind, ServeOptions, frame, serve};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EchoArgs {
    /// Text to echo back.
    text: String,
}

#[derive(Clone)]
struct EchoServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl EchoServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Echo the text back, framed as computed data.
    #[tool]
    async fn echo(&self, Parameters(args): Parameters<EchoArgs>) -> String {
        frame(FrameKind::Computed, &args.text)
    }

    /// Report whether an Authorization header reached the tool.
    #[tool]
    async fn whoami(&self, ctx: RequestContext<RoleServer>) -> String {
        let fwd = ForwardedHeaders::from_request_context(&ctx);
        match fwd.get("authorization") {
            Some(v) => format!("auth:{v}"),
            None => "auth:none".into(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for EchoServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn start() -> String {
    start_with(|o| o).await
}

async fn start_with(configure: impl FnOnce(ServeOptions) -> ServeOptions) -> String {
    let port = free_port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(serve(configure(ServeOptions::new(addr)), EchoServer::new));
    let base = format!("http://{addr}");
    for _ in 0..50 {
        if reqwest::get(format!("{base}/health")).await.is_ok() {
            return base;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("server did not start");
}

async fn rpc(base: &str, body: Value, auth: Option<&str>) -> Value {
    let mut req = reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .json(&body);
    if let Some(a) = auth {
        req = req.header("Authorization", a);
    }
    let resp = req.send().await.unwrap();
    assert!(resp.status().is_success(), "status {}", resp.status());
    let text = resp.text().await.unwrap();
    // JSON response, or one SSE `data:` line if the server chose a stream.
    let json = text
        .lines()
        .find_map(|l| l.strip_prefix("data:"))
        .unwrap_or(&text)
        .trim()
        .to_string();
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("bad body {text}: {e}"))
}

#[tokio::test]
async fn tools_list_and_call_over_stateless_http() {
    let base = start().await;

    let list = rpc(
        &base,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        None,
    )
    .await;
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        names.contains(&"echo") && names.contains(&"whoami"),
        "{list}"
    );

    let call = rpc(
        &base,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hello"}}}),
        None,
    )
    .await;
    let text = call["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("hello"), "{call}");
    assert_ne!(text, "hello", "output must be framed: {call}");

    let who = rpc(
        &base,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"whoami","arguments":{}}}),
        Some("Bearer t0ken"),
    )
    .await;
    assert_eq!(
        who["result"]["content"][0]["text"].as_str(),
        Some("auth:Bearer t0ken"),
        "{who}"
    );
}

#[tokio::test]
async fn host_validation_refuses_unlisted_hosts_when_enabled() {
    let base = start_with(|o| o.with_allowed_hosts(["mcp.example.com"])).await;
    let status = reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
        .send()
        .await
        .unwrap()
        .status();
    assert!(
        status.is_client_error(),
        "Host 127.0.0.1 must be refused, got {status}"
    );
}

#[tokio::test]
async fn origin_validation_refuses_foreign_origins_when_enabled() {
    let base = start_with(|o| o.with_allowed_origins(["https://app.example.com"])).await;
    let send = |origin: &'static str| {
        reqwest::Client::new()
            .post(format!("{base}/mcp"))
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-06-18")
            .header("Origin", origin)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
    };
    assert!(
        send("https://evil.example")
            .await
            .unwrap()
            .status()
            .is_client_error()
    );
    assert!(
        send("https://app.example.com")
            .await
            .unwrap()
            .status()
            .is_success()
    );
}
