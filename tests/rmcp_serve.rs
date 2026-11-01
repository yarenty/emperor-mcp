//! The rmcp path end to end: a `#[tool_router]` server mounted by [`emperor_mcp::serve`]
//! answers `tools/list` and `tools/call` over stateless Streamable HTTP, frames its output,
//! sees the forwarded `Authorization` header inside the tool, turns an upstream credential
//! rejection into the HTTP status, and answers both handshake families (P2 note in PROFILE.md).

use emperor_mcp::{
    ForwardedHeaders, FrameKind, Rejection, ServeOptions, credentials_rejected, frame, serve,
};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{ErrorData, ServerCapabilities, ServerConfig};
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
    #[tool(annotations(read_only_hint = true))]
    async fn echo(&self, Parameters(args): Parameters<EchoArgs>) -> String {
        frame(FrameKind::Computed, &args.text)
    }

    /// Report whether an Authorization header reached the tool.
    #[tool(annotations(read_only_hint = true))]
    async fn whoami(&self, ctx: RequestContext<RoleServer>) -> String {
        let fwd = ForwardedHeaders::from_request_context(&ctx);
        match fwd.get("authorization") {
            Some(v) => format!("auth:{v}"),
            None => "auth:none".into(),
        }
    }

    /// Call a pretend upstream that accepts only `Bearer good` and forbids `Bearer reader`.
    #[tool(annotations(read_only_hint = false, destructive_hint = false))]
    async fn upstream_call(&self, ctx: RequestContext<RoleServer>) -> Result<String, ErrorData> {
        let upstream_status = match ForwardedHeaders::from_request_context(&ctx).authorization() {
            Some("Bearer good") => 200,
            Some("Bearer reader") => 403,
            _ => 401,
        };
        match Rejection::from_status(upstream_status) {
            Some(rejection) => Err(credentials_rejected(&ctx, rejection, "pretend upstream")),
            None => Ok(frame(FrameKind::Computed, "done")),
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

async fn post_raw(base: &str, body: Value, auth: Option<&str>) -> reqwest::Response {
    let mut req = reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .json(&body);
    if let Some(a) = auth {
        req = req.header("Authorization", a);
    }
    req.send().await.unwrap()
}

fn call_upstream(id: u64) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"upstream_call","arguments":{}}})
}

#[tokio::test]
async fn upstream_credential_rejection_becomes_http_status() {
    let base = start().await;

    let ok = post_raw(&base, call_upstream(1), Some("Bearer good")).await;
    assert_eq!(ok.status(), 200);

    let expired = post_raw(&base, call_upstream(2), Some("Bearer expired")).await;
    assert_eq!(expired.status(), 401);
    assert_eq!(
        expired.headers()["www-authenticate"],
        r#"Bearer error="invalid_token""#
    );
    let body: Value = expired.json().await.unwrap();
    assert_eq!(body["id"], 2, "{body}");
    assert_eq!(
        body["error"]["code"],
        emperor_mcp::credentials::CREDENTIALS_REJECTED
    );
    assert_eq!(body["error"]["data"]["httpStatus"], 401);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("pretend upstream rejected the credentials"),
        "{body}"
    );

    let forbidden = post_raw(&base, call_upstream(3), Some("Bearer reader")).await;
    assert_eq!(forbidden.status(), 403);
    assert!(forbidden.headers().get("www-authenticate").is_none());

    // The slot is per request: the next good call is a plain 200 again.
    let again = post_raw(&base, call_upstream(4), Some("Bearer good")).await;
    assert_eq!(again.status(), 200);

    // Same on the 2026-07-28 per-request-version path.
    let mut modern = call_upstream(5);
    modern["params"]["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {"name":"t","version":"0"},
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let resp = reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "tools/call")
        .header("Mcp-Name", "upstream_call")
        .header("Authorization", "Bearer expired")
        .json(&modern)
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 401, "{body}");
}

#[tokio::test]
async fn answers_the_2025_06_18_initialize_handshake() {
    let base = start().await;
    let resp = post_raw(
        &base,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18",
            "capabilities":{},
            "clientInfo":{"name":"t","version":"0"}}}),
        None,
    )
    .await;
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("mcp-session-id").is_none());
    let init: Value = resp.json().await.unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18", "{init}");
    assert!(
        init["result"]["capabilities"]["tools"].is_object(),
        "{init}"
    );
}

#[tokio::test]
async fn answers_the_2026_07_28_server_discover_call() {
    let base = start().await;
    let resp = reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "server/discover")
        .json(
            &json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientInfo":{"name":"t","version":"0"},
            "io.modelcontextprotocol/clientCapabilities":{}}}}),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("mcp-session-id").is_none());
    let body: Value = resp.json().await.unwrap();
    let versions = body["result"]["supportedVersions"]
        .as_array()
        .unwrap_or_else(|| panic!("{body}"));
    assert!(versions.iter().any(|v| v == "2026-07-28"), "{body}");
    assert!(versions.iter().any(|v| v == "2025-06-18"), "{body}");
    assert!(
        body["result"]["capabilities"]["tools"].is_object(),
        "{body}"
    );
}

#[cfg(feature = "testkit")]
#[tokio::test]
async fn echo_server_tools_conform() {
    // From the tool router, as a server's unit test would do it.
    emperor_mcp::testkit::assert_tools(&EchoServer::new().tool_router.list_all());
    // From the wire, as an integration test against a running server would.
    let base = start().await;
    let list = rpc(
        &base,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        None,
    )
    .await;
    emperor_mcp::testkit::assert_tools_json(&list);
}
