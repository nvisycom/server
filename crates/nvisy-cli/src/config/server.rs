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
//! - `TLS_CERT_PATH` - Path to TLS certificate (optional, requires `tls` feature)
//! - `TLS_KEY_PATH` - Path to TLS private key (optional, requires `tls` feature)
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

use anyhow::{Result as AnyhowResult, anyhow};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::TRACING_TARGET_SERVER_STARTUP;

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
    pub tls_cert_path: Option<PathBuf>,

    /// Path to TLS private key file (PEM format).
    #[cfg(feature = "tls")]
    #[arg(long, env = "TLS_KEY_PATH")]
    pub tls_key_path: Option<PathBuf>,
}

const fn default_host() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}

impl ServerConfig {
    /// Validates configuration values.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Port is below 1024
    /// - Shutdown timeout is 0 or exceeds 300 seconds
    /// - Only one of TLS cert/key paths is provided (when TLS enabled)
    pub fn validate(&self) -> AnyhowResult<()> {
        if self.port < 1024 {
            return Err(anyhow!(
                "Port {} is below 1024. Use ports 1024-65535 to avoid requiring root privileges.",
                self.port
            ));
        }

        if self.shutdown_timeout == 0 || self.shutdown_timeout > 300 {
            return Err(anyhow!(
                "Shutdown timeout {} seconds is invalid. Must be between 1 and 300 seconds.",
                self.shutdown_timeout
            ));
        }

        #[cfg(feature = "tls")]
        {
            match (&self.tls_cert_path, &self.tls_key_path) {
                (Some(_), None) | (None, Some(_)) => {
                    return Err(anyhow!(
                        "Both TLS certificate and key paths must be provided together"
                    ));
                }
                _ => {}
            }
        }

        Ok(())
    }

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
        #[cfg(feature = "tls")]
        {
            self.tls_cert_path.is_some() && self.tls_key_path.is_some()
        }
        #[cfg(not(feature = "tls"))]
        {
            false
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 8080,
            shutdown_timeout: 30,
            #[cfg(feature = "tls")]
            tls_cert_path: None,
            #[cfg(feature = "tls")]
            tls_key_path: None,
        }
    }
}

/// Logs server configuration at startup.
pub fn log_server_config(config: &ServerConfig) {
    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        host = %config.host,
        port = config.port,
        tls = config.is_tls_enabled(),
        "server configuration"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_default_config() {
        let config = ServerConfig::default();
        assert!(config.validate().is_ok());
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
