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

    /// Accept only this Origin, with scheme, e.g. https://app.example.com (repeatable).
    #[arg(long = "allowed-origin", value_name = "ORIGIN")]
    pub allowed_origins: Vec<String>,
}

impl ServeArgs {
    /// The address to bind: `--bind`, or `default_bind` when it was not given.
    pub fn bind_or(&self, default_bind: SocketAddr) -> SocketAddr {
        self.bind.unwrap_or(default_bind)
    }

    /// [`ServeOptions`] for [`crate::serve`]: the bind address (`default_bind` when `--bind` was
    /// not given) and the Host/Origin allow-lists. The forwarded-header policy keeps its default
    /// (`Authorization` only); change it with [`ServeOptions::with_forward`].
    pub fn serve_options(&self, default_bind: SocketAddr) -> ServeOptions {
        ServeOptions::new(self.bind_or(default_bind))
            .with_allowed_hosts(self.allowed_hosts.iter().cloned())
            .with_allowed_origins(self.allowed_origins.iter().cloned())
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
}
