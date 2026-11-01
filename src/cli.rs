//! Shared command-line flags for server binaries (feature `cli`).
//!
//! Every server needs the same transport flags. Flatten [`ServeArgs`] into the server's own
//! `clap` parser so the flags are spelled the same everywhere:
//!
//! ```
//! use clap::Parser;
//! use emperor_mcp::cli::ServeArgs;
//!
//! #[derive(Parser)]
//! struct Cli {
//!     #[command(flatten)]
//!     serve: ServeArgs,
//!     /// The server's own flag.
//!     #[arg(long)]
//!     vault: Option<String>,
//! }
//!
//! let cli = Cli::parse_from(["my-mcp", "--bind", "0.0.0.0:8017", "--allowed-host", "mcp.example.com"]);
//! assert!(!cli.serve.stdio);
//! let options = cli.serve.serve_options("127.0.0.1:8017".parse().unwrap());
//! assert_eq!(options.bind.port(), 8017);
//! assert_eq!(options.allowed_hosts, ["mcp.example.com"]);
//! ```
//!
//! A server that can change things offers its writing tools only when the operator switches
//! writing on at start-up (Emperor Profile E11): a flag plus an environment variable, read with
//! [`parse_switch`] so an empty or falsey value leaves writing off:
//!
//! ```
//! use clap::{ArgAction, Parser};
//! use emperor_mcp::cli::parse_switch;
//!
//! #[derive(Parser)]
//! struct Cli {
//!     /// Offer the write tools (default: read-only, they are not listed).
//!     #[arg(long, env = "MY_MCP_WRITABLE", action = ArgAction::SetTrue, value_parser = parse_switch)]
//!     writable: bool,
//! }
//!
//! assert!(!Cli::parse_from(["my-mcp"]).writable);
//! assert!(Cli::parse_from(["my-mcp", "--writable"]).writable);
//! ```

use std::net::SocketAddr;

use crate::serve::ServeOptions;

/// Transport flags shared by every server: `--bind`, `--stdio`, `--allowed-host`,
/// `--allowed-origin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, clap::Args)]
pub struct ServeArgs {
    /// Address to listen on for Streamable HTTP, e.g. 127.0.0.1:8017 [default: the server's own].
    #[arg(long, value_name = "ADDR")]
    pub bind: Option<SocketAddr>,

    /// Speak MCP over stdin/stdout instead of HTTP (local and desktop use).
    #[arg(long, conflicts_with_all = ["bind", "allowed_hosts", "allowed_origins"])]
    pub stdio: bool,

    /// Accept only this Host header (repeatable). Without it, Host is not checked in-process.
    #[arg(long = "allowed-host", value_name = "HOST")]
    pub allowed_hosts: Vec<String>,

    /// Accept only this Origin, with scheme, e.g. `https://app.example.com` (repeatable).
    #[arg(long = "allowed-origin", value_name = "ORIGIN")]
    pub allowed_origins: Vec<String>,
}

impl ServeArgs {
    /// The address to bind: `--bind`, or `default_bind` when it was not given.
    pub fn bind_or(&self, default_bind: SocketAddr) -> SocketAddr {
        self.bind.unwrap_or(default_bind)
    }

    /// [`ServeOptions`] for [`crate::serve()`]: the bind address (`default_bind` when `--bind` was
    /// not given) and the Host/Origin allow-lists. The forwarded-header policy keeps its default
    /// (`Authorization` only); change it with [`ServeOptions::with_forward`].
    pub fn serve_options(&self, default_bind: SocketAddr) -> ServeOptions {
        ServeOptions::new(self.bind_or(default_bind))
            .with_allowed_hosts(self.allowed_hosts.iter().cloned())
            .with_allowed_origins(self.allowed_origins.iter().cloned())
    }
}

/// An on/off start-up switch from the environment, for a clap `value_parser`.
///
/// Off: empty (a catalog passing an unset parameter as `MY_MCP_WRITABLE=`), `false`, `0`, `no`,
/// `off`, `n`, `f`. On: `true`, `1`, `yes`, `on`, `y`, `t`. Case and surrounding spaces are
/// ignored. Anything else is refused, so a typo stops the server instead of switching writing on.
pub fn parse_switch(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "false" | "0" | "no" | "off" | "n" | "f" => Ok(false),
        "true" | "1" | "yes" | "on" | "y" | "t" => Ok(true),
        other => Err(format!("expected true/false, 1/0 or yes/no, got {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser, Debug)]
    struct Cli {
        #[command(flatten)]
        serve: ServeArgs,
    }

    fn parse(args: &[&str]) -> Result<ServeArgs, clap::Error> {
        Cli::try_parse_from(std::iter::once("t").chain(args.iter().copied())).map(|c| c.serve)
    }

    #[test]
    fn defaults_to_the_servers_own_bind() {
        let a = parse(&[]).unwrap();
        assert_eq!(a, ServeArgs::default());
        let o = a.serve_options("127.0.0.1:9000".parse().unwrap());
        assert_eq!(o.bind, "127.0.0.1:9000".parse().unwrap());
        assert!(o.allowed_hosts.is_empty() && o.allowed_origins.is_empty());
    }

    #[test]
    fn repeatable_hosts_and_origins() {
        let a = parse(&[
            "--bind",
            "0.0.0.0:8100",
            "--allowed-host",
            "localhost",
            "--allowed-host",
            "mcp.example.com",
            "--allowed-origin",
            "https://app.example.com",
        ])
        .unwrap();
        let o = a.serve_options("127.0.0.1:9000".parse().unwrap());
        assert_eq!(o.bind, "0.0.0.0:8100".parse().unwrap());
        assert_eq!(o.allowed_hosts, ["localhost", "mcp.example.com"]);
        assert_eq!(o.allowed_origins, ["https://app.example.com"]);
    }

    #[test]
    fn stdio_excludes_http_flags() {
        assert!(parse(&["--stdio"]).unwrap().stdio);
        assert!(parse(&["--stdio", "--bind", "127.0.0.1:1"]).is_err());
        assert!(parse(&["--stdio", "--allowed-host", "x"]).is_err());
        assert!(parse(&["--bind", "not-an-address"]).is_err());
    }

    #[test]
    fn switch_values() {
        for off in ["", " ", "false", "0", "No", "OFF", "n", "f"] {
            assert_eq!(parse_switch(off), Ok(false), "{off:?}");
        }
        for on in ["true", "1", "YES", " on ", "y", "T"] {
            assert_eq!(parse_switch(on), Ok(true), "{on:?}");
        }
        assert!(parse_switch("maybe").is_err());
        assert!(parse_switch("2").is_err());
    }
}
