//! HTTP server network and lifecycle configuration.
//!
//! This module provides configuration for the HTTP server's network binding,
//! TLS settings, and graceful shutdown behavior.
//!
//! # Environment Variables
//!
//! - `HOST` - Server host address (default: 127.0.0.1)
//! - `PORT` - Server port (default: 3000)
//! - `SHUTDOWN_TIMEOUT` - Graceful shutdown timeout in seconds (default: 30)
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

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
#[cfg(feature = "tls")]
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;
use serde::{Deserialize, Serialize};

use super::TRACING_TARGET_CONFIG;

/// HTTP server network and lifecycle configuration.
///
/// Controls how the server binds to network interfaces, handles TLS,
/// and performs graceful shutdown.
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct ServerConfig {
    /// Host address to bind the server to.
    ///
    /// Use "127.0.0.1" for localhost only, "0.0.0.0" for all interfaces.
    ///
    /// In production, consider binding to specific interfaces for security.
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    #[serde(default = "default_host")]
    pub host: IpAddr,

    /// TCP port number for the server to listen on.
    ///
    /// Must be in the range 1024-65535. Ports below 1024 require root privileges.
    ///
    /// Common choices: 3000 (development), 8080 (alternative HTTP), 443 (HTTPS).
    #[arg(short = 'p', long, env = "PORT", default_value_t = 3000)]
    pub port: u16,

    /// Maximum time in seconds to wait for graceful shutdown.
    ///
    /// During shutdown, the server stops accepting new connections and waits
    /// for existing requests to complete before forcefully terminating.
    #[arg(long, env = "SHUTDOWN_TIMEOUT", default_value_t = 30)]
    pub shutdown_timeout: u64,

    /// Path to TLS certificate file (PEM format).
    #[cfg(feature = "tls")]
    #[arg(long, env = "TLS_CERT_PATH")]
    pub tls_cert_path: PathBuf,

    /// Path to TLS private key file (PEM format).
    #[cfg(feature = "tls")]
    #[arg(long, env = "TLS_KEY_PATH")]
    pub tls_key_path: PathBuf,
}

const fn default_host() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}

impl ServerConfig {
    /// Returns the socket address for server binding.
    #[must_use]
    pub const fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Returns the graceful shutdown timeout as a `Duration`.
    #[must_use]
    pub const fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout)
    }

    /// Returns whether the server binds to all interfaces (0.0.0.0 or ::).
    #[must_use]
    pub const fn binds_to_all_interfaces(&self) -> bool {
        match self.host {
            IpAddr::V4(addr) => addr.is_unspecified(),
            IpAddr::V6(addr) => addr.is_unspecified(),
        }
    }

    /// Returns whether TLS is configured.
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
            shutdown_timeout_secs = self.shutdown_timeout,
            "Server configuration"
        );
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 8080,
            shutdown_timeout: 30,
            #[cfg(feature = "tls")]
            tls_cert_path: PathBuf::from("cert.pem"),
            #[cfg(feature = "tls")]
            tls_key_path: PathBuf::from("key.pem"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ServerConfig::default();
        assert!(config.binds_to_all_interfaces());
        assert_eq!(config.port, 8080);
        assert_eq!(config.shutdown_timeout, 30);
    }

    #[test]
    fn socket_addr_returns_correct_address() {
        let config = ServerConfig::default();
        let addr = config.socket_addr();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn binds_to_all_interfaces_detection() {
        let mut config = ServerConfig::default();
        assert!(config.binds_to_all_interfaces());

        config.host = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert!(!config.binds_to_all_interfaces());

        config.host = IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED);
        assert!(config.binds_to_all_interfaces());
    }
}
