//! Upstream credential rejection: tell the MCP client to sign in again.
//!
//! A server built on this framework forwards the caller's credentials to an upstream system
//! (see [`crate::headers`]). When that upstream answers `401` or `403`, an ordinary tool error
//! ("the call failed") gives the client nothing to act on. This module lets a tool say "the
//! upstream rejected the credentials" so the **MCP HTTP response itself** carries the status, and
//! a client can prompt the user to re-authenticate instead of showing a generic failure.
//!
//! ## How it flows (rmcp path, [`crate::serve`])
//!
//! 1. [`credential_rejection_middleware`] (mounted by [`crate::serve`]) puts an empty,
//!    request-scoped slot into the HTTP request's extensions.
//! 2. rmcp copies the request's [`http::request::Parts`] into the tool's `RequestContext`, so the
//!    tool reaches the same slot.
//! 3. The tool returns `Err(credentials_rejected(&ctx, Rejection::Unauthorized, "upstream"))`.
//!    That records the rejection in the slot and builds a JSON-RPC error
//!    ([`CREDENTIALS_REJECTED`], with `data.httpStatus`).
//! 4. On the way out the middleware sees the recorded rejection and rewrites the HTTP status of
//!    the (single JSON) reply to `401` or `403`. A `401` also carries
//!    `WWW-Authenticate: Bearer error="invalid_token"`. The body is still the JSON-RPC error.
//!
//! ## Hand-rolled path ([`crate::McpHandler`])
//!
//! Return [`rejection_envelope`] from the handler; [`crate::transport::http_router`] reads the
//! code and `data.httpStatus` and answers with that HTTP status.
//!
//! ## stdio
//!
//! There is no HTTP status on stdio. The client receives the JSON-RPC error: code
//! [`CREDENTIALS_REJECTED`], a message that says which upstream refused and what to do, and
//! `data.httpStatus`.
//!
//! ```ignore
//! #[tool(annotations(read_only_hint = true))]
//! async fn list_mail(&self, ctx: RequestContext<RoleServer>) -> Result<String, ErrorData> {
//!     let resp = self.upstream_call(&ctx).await?;
//!     if let Some(rejection) = Rejection::from_status(resp.status().as_u16()) {
//!         return Err(credentials_rejected(&ctx, rejection, "mail server"));
//!     }
//!     // ...
//! }
//! ```

use std::sync::{Arc, OnceLock};

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use http::{HeaderValue, StatusCode, header::WWW_AUTHENTICATE};
use rmcp::model::{ErrorCode, ErrorData};
use serde_json::{Value, json};

/// JSON-RPC error code for "the upstream rejected the forwarded credentials".
///
/// It sits in the JSON-RPC implementation-defined server error range (`-32000` to `-32099`)
/// and is specific to this framework; the authoritative signal over HTTP is the status code.
pub const CREDENTIALS_REJECTED: i32 = -32040;

/// Which way the upstream refused the credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// The upstream did not accept the credentials (missing, expired, revoked): HTTP `401`.
    Unauthorized,
    /// The credentials are valid but not allowed to do this: HTTP `403`.
    Forbidden,
}

impl Rejection {
    /// Map an upstream HTTP status: `401` and `403` are rejections, anything else is not.
    pub fn from_status(status: u16) -> Option<Self> {
        match status {
            401 => Some(Self::Unauthorized),
            403 => Some(Self::Forbidden),
            _ => None,
        }
    }

    /// The HTTP status the MCP response carries.
    pub fn status(self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
        }
    }

    fn message(self, upstream: &str) -> String {
        match self {
            Self::Unauthorized => format!(
                "{upstream} rejected the credentials (HTTP 401): sign in again or refresh the token"
            ),
            Self::Forbidden => format!(
                "{upstream} refused access with these credentials (HTTP 403): sign in with an \
                 account that is allowed to do this"
            ),
        }
    }

    fn data(self, upstream: &str) -> Value {
        json!({ "httpStatus": self.status().as_u16(), "upstream": upstream })
    }
}

/// Request-scoped slot the middleware creates and a tool fills. Cloning shares the slot, which
/// is what lets the copy of the request parts inside rmcp reach the middleware's original.
#[derive(Debug, Clone, Default)]
struct RejectionSlot(Arc<OnceLock<Rejection>>);

/// Record the rejection for this request (when the HTTP middleware runs) and build the JSON-RPC
/// error the tool returns.
///
/// Over HTTP through [`crate::serve`] the response status becomes `401`/`403`; elsewhere (stdio,
/// a router without the middleware) the client gets the JSON-RPC error alone.
pub fn credentials_rejected<R>(
    ctx: &rmcp::service::RequestContext<R>,
    rejection: Rejection,
    upstream: &str,
) -> ErrorData
where
    R: rmcp::service::ServiceRole,
{
    let slot = ctx.extensions.get::<RejectionSlot>().or_else(|| {
        ctx.extensions
            .get::<http::request::Parts>()
            .and_then(|p| p.extensions.get::<RejectionSlot>())
    });
    if let Some(slot) = slot {
        // First rejection wins; one request is one tool call.
        let _ = slot.0.set(rejection);
    }
    rejection_error(rejection, upstream)
}

/// The JSON-RPC error for a rejection, without touching any request state.
pub fn rejection_error(rejection: Rejection, upstream: &str) -> ErrorData {
    ErrorData::new(
        ErrorCode(CREDENTIALS_REJECTED),
        rejection.message(upstream),
        Some(rejection.data(upstream)),
    )
}

/// A complete JSON-RPC error envelope for the hand-rolled [`crate::McpHandler`] path.
/// [`crate::transport::http_router`] answers it with the matching HTTP status.
pub fn rejection_envelope(id: Value, rejection: Rejection, upstream: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": CREDENTIALS_REJECTED,
            "message": rejection.message(upstream),
            "data": rejection.data(upstream),
        }
    })
}

/// The rejection carried by a JSON-RPC reply, if it is a [`CREDENTIALS_REJECTED`] error.
pub fn rejection_in_envelope(envelope: &Value) -> Option<Rejection> {
    let error = envelope.get("error")?;
    if error.get("code")?.as_i64()? != i64::from(CREDENTIALS_REJECTED) {
        return None;
    }
    let status = error.pointer("/data/httpStatus")?.as_u64()?;
    Rejection::from_status(u16::try_from(status).ok()?)
}

/// Apply a rejection to an outgoing HTTP response: status, plus a Bearer challenge on `401`.
pub(crate) fn apply(rejection: Rejection, response: &mut Response) {
    *response.status_mut() = rejection.status();
    if rejection == Rejection::Unauthorized {
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static(r#"Bearer error="invalid_token""#),
        );
    }
}

/// Axum middleware that turns a tool's [`credentials_rejected`] into the HTTP status of the MCP
/// response. [`crate::serve`] mounts it; mount it yourself when you build your own router around
/// rmcp's `StreamableHttpService`.
pub async fn credential_rejection_middleware(mut request: Request, next: Next) -> Response {
    let slot = RejectionSlot::default();
    request.extensions_mut().insert(slot.clone());
    let mut response = next.run(request).await;
    if let Some(rejection) = slot.0.get().copied()
        && response.status().is_success()
    {
        apply(rejection, &mut response);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert_eq!(Rejection::from_status(401), Some(Rejection::Unauthorized));
        assert_eq!(Rejection::from_status(403), Some(Rejection::Forbidden));
        assert_eq!(Rejection::from_status(404), None);
        assert_eq!(Rejection::from_status(500), None);
        assert_eq!(Rejection::Unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(Rejection::Forbidden.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn error_is_clear_without_http() {
        let e = rejection_error(Rejection::Unauthorized, "mail server");
        assert_eq!(e.code, ErrorCode(CREDENTIALS_REJECTED));
        assert!(e.message.contains("mail server rejected the credentials"));
        assert!(e.message.contains("sign in again"));
        assert_eq!(e.data.as_ref().unwrap()["httpStatus"], 401);
    }

    #[test]
    fn envelope_round_trips() {
        let env = rejection_envelope(json!(7), Rejection::Forbidden, "wiki");
        assert_eq!(env["id"], 7);
        assert_eq!(rejection_in_envelope(&env), Some(Rejection::Forbidden));
        let other = json!({"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"x"}});
        assert_eq!(rejection_in_envelope(&other), None);
        let ok = json!({"jsonrpc":"2.0","id":1,"result":{}});
        assert_eq!(rejection_in_envelope(&ok), None);
    }
}
