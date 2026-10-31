//! `doctor`: an environment check every server binary ships as a subcommand.
//!
//! A server's `doctor` builds a [`Report`] from [`Check`]s (its own, plus the generic ones here:
//! [`port_free`], [`env_set`], [`tcp_reachable`]), prints it, and exits with
//! [`Report::exit_code`], so scripts and container health checks can rely on it.
//!
//! ```no_run
//! use emperor_mcp::doctor::{Report, env_set, port_free};
//! let mut report = Report::new("my-mcp", env!("CARGO_PKG_VERSION"));
//! report.push(port_free("127.0.0.1:8017".parse().unwrap()));
//! report.push(env_set("MY_API_TOKEN", true));
//! report.print();
//! std::process::exit(report.exit_code());
//! ```
//!
//! Checks never print secret values: [`env_set`] with `secret = true` reports only that the
//! variable is set and how long it is.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

/// Outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Works.
    Pass,
    /// Works, but something should be looked at (does not fail the report).
    Warn,
    /// Broken: the server will not work as configured.
    Fail,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pass => "ok  ",
            Self::Warn => "warn",
            Self::Fail => "FAIL",
        })
    }
}

/// One line of a doctor report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// What was checked, e.g. `imap login`.
    pub name: String,
    /// The outcome.
    pub status: Status,
    /// What was found, or what to do about it.
    pub detail: String,
}

impl Check {
    /// A passing check.
    pub fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(name, Status::Pass, detail)
    }

    /// A warning.
    pub fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(name, Status::Warn, detail)
    }

    /// A failure.
    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(name, Status::Fail, detail)
    }

    fn new(name: impl Into<String>, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status,
            detail: detail.into(),
        }
    }
}

/// A doctor report: a header line and a list of checks.
#[derive(Debug, Clone)]
pub struct Report {
    server: String,
    version: String,
    checks: Vec<Check>,
}

impl Report {
    /// An empty report for `server` at `version`.
    pub fn new(server: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            version: version.into(),
            checks: Vec::new(),
        }
    }

    /// Add a check.
    pub fn push(&mut self, check: Check) -> &mut Self {
        self.checks.push(check);
        self
    }

    /// The checks so far.
    pub fn checks(&self) -> &[Check] {
        &self.checks
    }

    /// True when no check failed (warnings allowed).
    pub fn is_ok(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Fail)
    }

    /// `0` when [`Self::is_ok`], `1` otherwise.
    pub fn exit_code(&self) -> i32 {
        if self.is_ok() { 0 } else { 1 }
    }

    /// Print the report to stdout.
    pub fn print(&self) {
        print!("{self}");
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {} doctor", self.server, self.version)?;
        let width = self.checks.iter().map(|c| c.name.len()).max().unwrap_or(0);
        for c in &self.checks {
            writeln!(f, "  [{}] {:<width$}  {}", c.status, c.name, c.detail)?;
        }
        let failed = self
            .checks
            .iter()
            .filter(|c| c.status == Status::Fail)
            .count();
        let warned = self
            .checks
            .iter()
            .filter(|c| c.status == Status::Warn)
            .count();
        if failed == 0 {
            writeln!(f, "all good ({warned} warning(s))")
        } else {
            writeln!(f, "{failed} check(s) failed, {warned} warning(s)")
        }
    }
}

/// The address can be bound (nothing else is listening there).
pub fn port_free(addr: SocketAddr) -> Check {
    match std::net::TcpListener::bind(addr) {
        Ok(_) => Check::pass("listen address", format!("{addr} is free")),
        Err(e) => Check::fail(
            "listen address",
            format!("{addr} cannot be bound ({e}); is the server already running?"),
        ),
    }
}

/// An environment variable is set and non-empty. With `secret`, only its length is shown.
pub fn env_set(var: &str, secret: bool) -> Check {
    match std::env::var(var) {
        Ok(v) if !v.trim().is_empty() => {
            let shown = if secret {
                format!("set ({} chars)", v.chars().count())
            } else {
                format!("= {v}")
            };
            Check::pass(var, shown)
        }
        Ok(_) => Check::fail(var, "set but empty"),
        Err(_) => Check::fail(var, "not set"),
    }
}

/// A TCP connection to `host:port` opens within `timeout`.
pub async fn tcp_reachable(name: &str, host: &str, port: u16, timeout: Duration) -> Check {
    let target = format!("{host}:{port}");
    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&target)).await {
        Ok(Ok(_)) => Check::pass(name, format!("{target} reachable")),
        Ok(Err(e)) => Check::fail(name, format!("{target}: {e}")),
        Err(_) => Check::fail(name, format!("{target}: no answer within {timeout:?}")),
    }
}

/// Most tools a server may expose (MCP_REQUIREMENTS §11): a model picks well from a handful of
/// tools and badly from thirty.
pub const MAX_TOOLS: usize = 8;

/// The server's tool count is within [`MAX_TOOLS`].
pub fn tool_budget(count: usize) -> Check {
    if count == 0 {
        Check::fail("tool budget", "no tools registered")
    } else if count > MAX_TOOLS {
        Check::fail(
            "tool budget",
            format!("{count} tools, the limit is {MAX_TOOLS}; split the server"),
        )
    } else {
        Check::pass(
            "tool budget",
            format!("{count} of at most {MAX_TOOLS} tools"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_fails_only_on_fail() {
        let mut r = Report::new("t", "0.0.0");
        r.push(Check::pass("a", "fine"))
            .push(Check::warn("b", "hmm"));
        assert!(r.is_ok());
        assert_eq!(r.exit_code(), 0);
        r.push(Check::fail("c", "broken"));
        assert!(!r.is_ok());
        assert_eq!(r.exit_code(), 1);
        let text = r.to_string();
        assert!(text.contains("[FAIL] c"), "{text}");
        assert!(text.contains("1 check(s) failed, 1 warning(s)"), "{text}");
    }

    #[test]
    fn secret_env_is_never_printed() {
        // SAFETY: test-only variable name, set before any reader in this test.
        unsafe { std::env::set_var("EMPEROR_DOCTOR_TEST_SECRET", "hunter2") };
        let c = env_set("EMPEROR_DOCTOR_TEST_SECRET", true);
        assert_eq!(c.status, Status::Pass);
        assert!(!c.detail.contains("hunter2"));
        assert_eq!(c.detail, "set (7 chars)");
        assert_eq!(
            env_set("EMPEROR_DOCTOR_TEST_UNSET", false).status,
            Status::Fail
        );
    }

    #[test]
    fn tool_budget_bounds() {
        assert_eq!(tool_budget(0).status, Status::Fail);
        assert_eq!(tool_budget(6).status, Status::Pass);
        assert_eq!(tool_budget(MAX_TOOLS + 1).status, Status::Fail);
    }

    #[test]
    fn busy_port_fails() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        assert_eq!(port_free(addr).status, Status::Fail);
    }

    #[tokio::test]
    async fn reachable_and_unreachable() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let ok = tcp_reachable("svc", "127.0.0.1", port, Duration::from_secs(2)).await;
        assert_eq!(ok.status, Status::Pass);
        drop(l);
        let gone = tcp_reachable("svc", "127.0.0.1", port, Duration::from_secs(2)).await;
        assert_eq!(gone.status, Status::Fail);
    }
}
