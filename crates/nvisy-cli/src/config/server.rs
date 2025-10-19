//! HTTP server configuration.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Result as AnyhowResult, anyhow};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::TRACING_TARGET_CONFIG;

/// HTTP server configuration.
///
/// This struct contains all configuration options for the HTTP server including
/// network binding, timeouts, and performance tuning parameters.
///
/// # Environment Variables
///
/// All configuration options can be set via environment variables:
/// - `HOST` - Server host address (default: 127.0.0.1)
/// - `PORT` - Server port (default: 3000, valid range: 1024-65535)
/// - `REQUEST_TIMEOUT` - Request processing timeout in seconds (default: 30, max: 300)
/// - `SHUTDOWN_TIMEOUT` - Graceful shutdown timeout in seconds (default: 30, max: 300)
/// - `CORS_ALLOWED_ORIGINS` - Comma-separated list of allowed CORS origins
///
/// # Examples
///
/// ```bash
/// # Using CLI arguments
/// nvisy-cli --host 0.0.0.0 --port 8080
///
/// # Using environment variables
/// HOST=0.0.0.0 PORT=8080 nvisy-cli
/// ```
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct ServerConfig {
    /// Host address to bind the server to.
    ///
    /// Use "127.0.0.1" for localhost only, "0.0.0.0" for all interfaces.
    /// In production, consider binding to specific interfaces for security.
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    #[serde(default = "default_host")]
    pub host: IpAddr,

    /// TCP port number for the server to listen on.
    ///
    /// Must be in the range 1024-65535. Ports below 1024 require root privileges.
    /// Common choices: 3000 (development), 8080 (alternative HTTP), 443 (HTTPS).
    #[arg(short = 'p', long, env = "PORT", default_value_t = 3000)]
    pub port: u16,

    /// Maximum time in seconds to wait for a request to complete.
    ///
    /// This includes time to read the request, process it, and send the response.
    /// Requests exceeding this timeout will be terminated with a 408 Request Timeout.
    /// Valid range: 1-300 seconds.
    #[arg(long, env = "REQUEST_TIMEOUT", default_value_t = 30)]
    pub request_timeout: u64,

    /// Maximum time in seconds to wait for graceful shutdown.
    ///
    /// During shutdown, the server will stop accepting new connections and wait
    /// up to this duration for existing requests to complete before forcefully
    /// terminating them. Valid range: 1-300 seconds.
    #[arg(long, env = "SHUTDOWN_TIMEOUT", default_value_t = 30)]
    pub shutdown_timeout: u64,

    /// List of allowed CORS origins.
    ///
    /// If empty, localhost origins will be used for development.
    /// In production, specify the exact origins that should be allowed.
    /// Example: https://nvisy.com,https://app.nvisy.com
    #[arg(long, env = "CORS_ALLOWED_ORIGINS", value_delimiter = ',')]
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,

    /// Path to TLS certificate file (PEM format).
    ///
    /// Only used when TLS feature is enabled.
    #[cfg(feature = "tls")]
    #[arg(long, env = "TLS_CERT_PATH")]
    pub tls_cert_path: Option<std::path::PathBuf>,

    /// Path to TLS private key file (PEM format).
    ///
    /// Only used when TLS feature is enabled.
    #[cfg(feature = "tls")]
    #[arg(long, env = "TLS_KEY_PATH")]
    pub tls_key_path: Option<std::path::PathBuf>,
}

/// Default host address for development.
fn default_host() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

impl ServerConfig {
    /// Validates all configuration values and returns errors for invalid settings.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration value is outside its valid range:
    /// - Port must be 1024-65535
    /// - Request timeout must be 1-300 seconds
    /// - Shutdown timeout must be 1-300 seconds
    /// - TLS paths must be provided together (when TLS is enabled)
    pub fn validate(&self) -> AnyhowResult<()> {
        // Validate port range
        if self.port < 1024 {
            return Err(anyhow!(
                "Port {} is below 1024. Use ports 1024-65535 to avoid requiring root privileges.",
                self.port
            ));
        }

        // Validate request timeout
        if self.request_timeout == 0 || self.request_timeout > 300 {
            return Err(anyhow!(
                "Request timeout {} seconds is invalid. Must be between 1 and 300 seconds.",
                self.request_timeout
            ));
        }

        // Validate shutdown timeout
        if self.shutdown_timeout == 0 || self.shutdown_timeout > 300 {
            return Err(anyhow!(
                "Shutdown timeout {} seconds is invalid. Must be between 1 and 300 seconds.",
                self.shutdown_timeout
            ));
        }

        // Validate TLS configuration
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

    /// Returns the complete socket address for server binding.
    ///
    /// Combines the configured host and port into a `SocketAddr`.
    #[must_use]
    pub const fn server_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Returns the request processing timeout as a `Duration`.
    #[must_use]
    pub const fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout)
    }

    /// Returns the graceful shutdown timeout as a `Duration`.
    #[must_use]
    pub const fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout)
    }

    /// Returns whether the server is configured to bind to all interfaces.
    ///
    /// This is true when the host is set to "0.0.0.0" (IPv4) or "::" (IPv6).
    #[must_use]
    pub const fn binds_to_all_interfaces(&self) -> bool {
        match self.host {
            IpAddr::V4(addr) => addr.is_unspecified(),
            IpAddr::V6(addr) => addr.is_unspecified(),
        }
    }

    /// Returns whether this is a development configuration.
    #[must_use]
    pub fn is_development(&self) -> bool {
        matches!(self.host, IpAddr::V4(addr) if addr.is_loopback()) && self.port == 3000
    }

    /// Returns whether TLS is configured.
    #[must_use]
    #[cfg(feature = "tls")]
    pub const fn is_tls_enabled(&self) -> bool {
        self.tls_cert_path.is_some() && self.tls_key_path.is_some()
    }

    /// Creates a production-ready configuration with secure defaults.
    #[must_use]
    pub fn production() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8080,
            request_timeout: 60,
            shutdown_timeout: 60,
            cors_allowed_origins: vec![
                "https://nvisy.com".to_string(),
                "https://app.nvisy.com".to_string(),
            ],
            #[cfg(feature = "tls")]
            tls_cert_path: None,
            #[cfg(feature = "tls")]
            tls_key_path: None,
        }
    }
}

impl Default for ServerConfig {
    /// Creates a development-friendly configuration with safe defaults.
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 3000,
            request_timeout: 30,
            shutdown_timeout: 30,
            cors_allowed_origins: Vec::new(),
            #[cfg(feature = "tls")]
            tls_cert_path: None,
            #[cfg(feature = "tls")]
            tls_key_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_default_config() {
        let config = ServerConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.is_development());
        assert!(!config.binds_to_all_interfaces());
    }

    #[test]
    fn validate_production_config() {
        let config = ServerConfig::production();
        assert!(config.validate().is_ok());
        assert!(!config.is_development());
    }

    #[test]
    fn reject_privileged_ports() {
        let mut config = ServerConfig::default();
        config.port = 80;
        assert!(config.validate().is_err());
    }

    #[test]
    fn reject_invalid_timeouts() {
        let mut config = ServerConfig::default();

        config.request_timeout = 0;
        assert!(config.validate().is_err());

        config.request_timeout = 301;
        assert!(config.validate().is_err());

        config.request_timeout = 60;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn server_addr_returns_correct_socket() {
        let config = ServerConfig::default();
        let addr = config.server_addr();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(addr.port(), 3000);
    }
}

/// Logs server configuration details with appropriate tracing.
///
/// This function logs essential server configuration information at startup,
/// including host, port, and TLS status when applicable.
pub fn log_server_config(config: &ServerConfig) {
    #[cfg(feature = "tls")]
    {
        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            host = %config.host,
            port = config.port,
            tls_enabled = config.is_tls_enabled(),
            development_mode = config.is_development(),
            "Server configured successfully"
        );
    }

    #[cfg(not(feature = "tls"))]
    {
        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            host = %config.host,
            port = config.port,
            development_mode = config.is_development(),
            "Server configured successfully"
        );
    }
}
