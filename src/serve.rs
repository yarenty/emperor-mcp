//!
//! Streamable-HTTP server bootstrap.
//!
//! [`serve`] wires together:
//!   * the rmcp `StreamableHttpService` mounted at `/mcp`,
//!   * the [`super::headers::forward_headers_middleware`] that captures
//!     per-user credential headers for forwarding,
//!   * a `/health` endpoint,
//!   * graceful shutdown on Ctrl-C / cancellation.
//!

use std::net::SocketAddr;

use rmcp::handler::server::ServerHandler;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::tower::StreamableHttpService;
use tokio_util::sync::CancellationToken;

use crate::headers::{ForwardConfig, forward_headers_middleware};

const LEGACY_SESSION_MODE: bool = false;

/// Options for [`serve`].
pub struct ServeOptions {
    /// Address to bind, e.g. `0.0.0.0:8000`.
    pub bind: SocketAddr,
    /// Which inbound headers to forward to the upstream service per request.
    pub forward: ForwardConfig,
    /// In-process `Host` validation (DNS-rebinding protection). Empty = off, for deployments
    /// behind a validating ingress (Emperor Profile E5).
    pub allowed_hosts: Vec<String>,
    /// In-process `Origin` validation, entries with a scheme (`https://app.example`). Empty = off.
    pub allowed_origins: Vec<String>,
}

impl ServeOptions {
    /// Build options from a bind address and the common "forward only
    /// Authorization" header policy.
    pub fn new(bind: SocketAddr) -> Self {
        Self {
            bind,
            forward: ForwardConfig::authorization_only(),
            allowed_hosts: Vec::new(),
            allowed_origins: Vec::new(),
        }
    }

    /// Override the forwarded-header allow-list (e.g. for MCPs that use
    /// `X-*` credential headers instead of / in addition to Authorization).
    pub fn with_forward(mut self, forward: ForwardConfig) -> Self {
        self.forward = forward;
        self
    }

    /// Validate the `Host` header in-process (e.g. `["localhost", "mcp.example.com"]`), for
    /// deployments without a validating ingress in front.
    pub fn with_allowed_hosts<I, S>(mut self, hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_hosts = hosts.into_iter().map(Into::into).collect();
        self
    }

    /// Validate the `Origin` header in-process (entries include the scheme). Requests without
    /// an `Origin` header (non-browser clients) still pass.
    pub fn with_allowed_origins<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_origins = origins.into_iter().map(Into::into).collect();
        self
    }
}

/// Run a first-party MCP server over streamable-HTTP until shutdown.
///
/// `make_handler` is called once per session to build a fresh handler
/// instance (rmcp's session model). The handler must implement
/// [`ServerHandler`] (typically via `#[tool_router]` + `#[tool]`).
///
/// Per-user credential headers named in `options.forward` are captured by
/// middleware and made available to tool handlers via
/// [`crate::headers::ForwardedHeaders::from_request_context`].
pub async fn serve<H, F>(options: ServeOptions, make_handler: F) -> anyhow::Result<()>
where
    H: ServerHandler + Send + 'static,
    F: Fn() -> H + Send + Sync + 'static,
{
    let ct = CancellationToken::new();

    let mut config = StreamableHttpServerConfig::default()
        // Stateless: no `initialize`/`Mcp-Session-Id` handshake. Each tool call
        // is a self-contained POST so per-request credential forwarding works
        // and tool discovery (unauthenticated `tools/list`) still succeeds.
        .with_legacy_session_mode(LEGACY_SESSION_MODE)
        // Return plain JSON (no SSE framing) for simple request/response tools,
        // which is what the gateway's sessionless path expects.
        .with_json_response(!LEGACY_SESSION_MODE)
        .with_cancellation_token(ct.child_token());
    // Host/Origin validation: off by default (a validating ingress does it, Emperor Profile E5),
    // in-process when the options name hosts or origins.
    config = if options.allowed_hosts.is_empty() {
        config.disable_allowed_hosts()
    } else {
        config.with_allowed_hosts(options.allowed_hosts.clone())
    };
    config = if options.allowed_origins.is_empty() {
        config.disable_allowed_origins()
    } else {
        config.with_allowed_origins(options.allowed_origins.clone())
    };

    let service = StreamableHttpService::new(
        move || Ok(make_handler()),
        LocalSessionManager::default().into(),
        config,
    );

    let app = axum::Router::new()
        .route("/health", axum::routing::get(health))
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn_with_state(
            options.forward.clone(),
            forward_headers_middleware,
        ));

    let listener = tokio::net::TcpListener::bind(options.bind).await?;
    tracing::info!(bind = %options.bind, "MCP streamable-HTTP server listening on /mcp");

    let shutdown = {
        let ct = ct.clone();
        async move {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutdown signal received");
            ct.cancel();
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

/// Minimal liveness endpoint.
async fn health() -> &'static str {
    "OK"
}
