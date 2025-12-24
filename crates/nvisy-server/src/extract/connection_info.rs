//! Connection information extractor for HTTP requests.
//!
//! This module provides the [`AppConnectInfo`] extractor for obtaining detailed
//! information about client connections in Axum handlers. It captures network
//! addresses, connection timing, and provides utilities for IP classification
//! and security analysis.

use std::net::{IpAddr, SocketAddr};
use std::ops::Deref;
use std::time::SystemTime;

use axum::extract::FromRequestParts;
use axum::extract::connect_info::Connected;
use axum::serve::IncomingStream;
use tokio::net::TcpListener;

/// Wrapper around [`axum_client_ip::ClientIp`] that implements [`aide::OperationInput`].
///
/// This allows the extractor to be used with aide's OpenAPI generation.
#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub IpAddr);

impl Deref for ClientIp {
    type Target = IpAddr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = <axum_client_ip::ClientIp as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let axum_client_ip::ClientIp(ip) =
            axum_client_ip::ClientIp::from_request_parts(parts, state).await?;
        Ok(Self(ip))
    }
}

impl aide::OperationInput for ClientIp {}

/// Enhanced connection information extractor for incoming HTTP requests.
///
/// This extractor provides comprehensive information about client connections,
/// including network addresses, connection timing, and security metadata.
/// It can be used for logging, rate limiting, geolocation, and security analysis.
///
/// # Features
///
/// - Client socket address (IP + port)
/// - Connection establishment timestamp
/// - IP address classification (IPv4/IPv6, private/public)
/// - Real IP detection (handles proxy headers)
/// - Connection metadata for security analysis
///
/// # Security Considerations
///
/// When deployed behind a proxy or load balancer, the `addr` field will
/// contain the proxy's address, not the original client IP. For production
/// deployments, consider using middleware to extract real client IPs from
/// proxy headers (X-Forwarded-For, X-Real-IP, etc.).
#[derive(Debug, Clone)]
#[must_use]
pub struct AppConnectInfo {
    /// The socket address (IP + port) of the connecting client.
    ///
    /// Note: When behind a proxy, this will be the proxy's address.
    pub addr: SocketAddr,

    /// Timestamp when the connection was established.
    ///
    /// This can be used for connection duration tracking and security analysis.
    pub connected_at: SystemTime,

    /// Optional real client IP address extracted from proxy headers.
    ///
    /// This field should be populated by middleware that processes
    /// X-Forwarded-For, X-Real-IP, or similar proxy headers.
    pub real_ip: Option<IpAddr>,
}

impl AppConnectInfo {
    /// Creates a new `AppConnectInfo` with the current timestamp.
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            connected_at: SystemTime::now(),
            real_ip: None,
        }
    }

    /// Creates a new `AppConnectInfo` with a real IP address override.
    pub fn with_real_ip(addr: SocketAddr, real_ip: IpAddr) -> Self {
        Self {
            addr,
            connected_at: SystemTime::now(),
            real_ip: Some(real_ip),
        }
    }

    /// Returns the client's IP address.
    ///
    /// If a real IP was detected (from proxy headers), returns that.
    /// Otherwise, returns the direct connection IP.
    #[inline]
    pub fn client_ip(&self) -> IpAddr {
        self.real_ip.unwrap_or_else(|| self.addr.ip())
    }

    /// Returns the client's port number from the direct connection.
    #[inline]
    pub fn client_port(&self) -> u16 {
        self.addr.port()
    }

    /// Returns `true` if the client IP is a private/internal address.
    ///
    /// This includes loopback addresses, private IPv4 ranges (10.0.0.0/8,
    /// 172.16.0.0/12, 192.168.0.0/16), and IPv6 private addresses.
    #[inline]
    pub fn is_private_ip(&self) -> bool {
        match self.client_ip() {
            IpAddr::V4(ipv4) => {
                ipv4.is_private()
                    || ipv4.is_loopback()
                    || ipv4.is_link_local()
                    || ipv4.is_unspecified()
            }
            IpAddr::V6(ipv6) => {
                ipv6.is_loopback() || ipv6.is_unspecified() || ipv6.segments()[0] & 0xfe00 == 0xfc00 // Unique local addresses
            }
        }
    }

    /// Returns `true` if the client IP is a public/external address.
    #[inline]
    pub fn is_public_ip(&self) -> bool {
        !self.is_private_ip()
    }

    /// Returns `true` if the connection is from localhost.
    #[inline]
    pub fn is_localhost(&self) -> bool {
        self.client_ip().is_loopback()
    }

    /// Returns `true` if the client is connecting via IPv4.
    #[inline]
    pub fn is_ipv4(&self) -> bool {
        matches!(self.client_ip(), IpAddr::V4(_))
    }

    /// Returns `true` if the client is connecting via IPv6.
    #[inline]
    pub fn is_ipv6(&self) -> bool {
        matches!(self.client_ip(), IpAddr::V6(_))
    }

    /// Returns the duration since the connection was established.
    ///
    /// Returns `None` if the system clock has moved backward.
    pub fn connection_duration(&self) -> Option<std::time::Duration> {
        SystemTime::now().duration_since(self.connected_at).ok()
    }

    /// Returns a string representation suitable for logging.
    ///
    /// Includes both the direct address and real IP (if different).
    pub fn to_log_string(&self) -> String {
        match self.real_ip {
            Some(real_ip) if real_ip != self.addr.ip() => {
                format!("{} (via {})", real_ip, self.addr.ip())
            }
            _ => self.addr.to_string(),
        }
    }
}

impl Connected<IncomingStream<'_, TcpListener>> for AppConnectInfo {
    fn connect_info(stream: IncomingStream<'_, TcpListener>) -> Self {
        let addr = SocketAddr::connect_info(stream);
        Self::new(addr)
    }
}

// https://github.com/programatik29/axum-server/issues/12
impl Connected<SocketAddr> for AppConnectInfo {
    fn connect_info(addr: SocketAddr) -> Self {
        Self::new(addr)
    }
}
