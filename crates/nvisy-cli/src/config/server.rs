//! HTTP server network and lifecycle configuration.
//!
//! This module provides configuration for the HTTP server's network binding,
//! TLS settings, and graceful shutdown behavior.
//!
//! # Environment Variables
//!
//! - `HOST` - Server host address (default: 127.0.0.1)
//! - `PORT` - Server port (default: 8080)
//! - `SHUTDOWN_TIMEOUT` - Graceful shutdown timeout (default: 30s)
//! - `TLS_CERT_PATH` - Path to TLS certificate (required when `tls` feature enabled)
//! - `TLS_KEY_PATH` - Path to TLS private key (required when `tls` feature enabled)
//!
//! # Example
//!
//! ```bash
//! # Bind to all interfaces on port 8080
//! nvisy-cli --host 0.0.0.0 --port 8080
//!
//! # With TLS (requires tls feature)
//! nvisy-cli --tls-cert-path ./cert.pem --tls-key-path ./key.pem
//! ```

use std::net::{IpAddr, SocketAddr};
#[cfg(feature = "tls")]
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;

use super::TRACING_TARGET_CONFIG;

/// HTTP server network and lifecycle configuration.
///
/// Controls how the server binds to network interfaces, handles TLS,
/// and performs graceful shutdown.
#[derive(Debug, Clone, Args)]
#[must_use = "config does nothing unless you use it"]
pub struct ServerConfig {
    /// Host address to bind the server to.
    ///
    /// Use "127.0.0.1" for localhost only, "0.0.0.0" for all interfaces.
    ///
    /// In production, consider binding to specific interfaces for security.
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    pub host: IpAddr,

    /// TCP port number for the server to listen on.
    ///
    /// Must be in the range 1024-65535. Ports below 1024 require root privileges.
    ///
    /// Common choices: 3000 (development), 8080 (alternative HTTP), 443 (HTTPS).
    #[arg(short = 'p', long, env = "PORT", default_value_t = 8080)]
    pub port: u16,

    /// Maximum time to wait for graceful shutdown (e.g. `30s`, `2m`).
    ///
    /// During shutdown, the server stops accepting new connections and waits
    /// for existing requests to complete before forcefully terminating.
    #[arg(
        long,
        env = "SHUTDOWN_TIMEOUT",
        default_value = "30s",
        value_parser = nvisy_core::parse_duration,
    )]
    pub shutdown_timeout: Duration,

    /// Path to TLS certificate file (PEM format).
    #[cfg(feature = "tls")]
    #[arg(long, env = "TLS_CERT_PATH")]
    pub tls_cert_path: PathBuf,

    /// Path to TLS private key file (PEM format).
    #[cfg(feature = "tls")]
    #[arg(long, env = "TLS_KEY_PATH")]
    pub tls_key_path: PathBuf,
}

impl ServerConfig {
    /// A loopback config on an OS-assigned port, for tests.
    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            host: IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            port: 0,
            shutdown_timeout: Duration::from_secs(30),
            #[cfg(feature = "tls")]
            tls_cert_path: PathBuf::from("cert.pem"),
            #[cfg(feature = "tls")]
            tls_key_path: PathBuf::from("key.pem"),
        }
    }

    /// Returns the socket address for server binding.
    #[must_use]
    pub const fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Returns the graceful shutdown timeout.
    #[must_use]
    pub const fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    /// Returns whether the server binds to all interfaces (0.0.0.0 or ::).
    #[must_use]
    pub const fn binds_to_all_interfaces(&self) -> bool {
        self.host.is_unspecified()
    }

    /// Returns whether TLS is configured.
    // reason: reads as a config accessor alongside the other `&self` queries.
    #[allow(clippy::unused_self)]
    #[must_use]
    pub const fn is_tls_enabled(&self) -> bool {
        cfg!(feature = "tls")
    }

    /// Logs server configuration at info level.
    pub fn log(&self) {
        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            host = %self.host,
            port = self.port,
            tls = self.is_tls_enabled(),
            shutdown_timeout = ?self.shutdown_timeout,
            "Server configuration"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    #[test]
    fn socket_addr_joins_host_and_port() {
        let mut config = ServerConfig::test();
        config.port = 8080;
        assert_eq!(
            config.socket_addr(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080)
        );
    }

    #[test]
    fn binds_to_all_interfaces_detection() {
        let mut config = ServerConfig::test();
        assert!(!config.binds_to_all_interfaces());
        config.host = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        assert!(config.binds_to_all_interfaces());
        config.host = IpAddr::V6(Ipv6Addr::UNSPECIFIED);
        assert!(config.binds_to_all_interfaces());
    }
}
