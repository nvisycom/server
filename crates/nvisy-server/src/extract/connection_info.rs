//! Connection information extractor for HTTP requests.
//!
//! This module provides the [`AppConnectInfo`] extractor for obtaining detailed
//! information about client connections in Axum handlers. It captures network
//! addresses, connection timing, and provides utilities for IP classification
//! and security analysis.

use std::net::{IpAddr, SocketAddr};
use std::ops::Deref;
use std::time::{Duration, SystemTime};

use axum::extract::FromRequestParts;
use axum::extract::connect_info::Connected;
use axum::http::request::Parts;
use axum::serve::IncomingStream;
use tokio::net::TcpListener;

/// Client IP address extractor with OpenAPI support.
///
/// This is a wrapper around [`axum_client_ip::ClientIp`] that adds
/// [`aide::OperationInput`] implementation for OpenAPI schema generation.
/// It extracts the client's IP address from the request, handling proxy
/// headers like `X-Forwarded-For` when configured.
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

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
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
                ipv6.is_loopback()
                    || ipv6.is_unspecified()
                    || ipv6.is_unique_local()        // fc00::/7
                    || ipv6.is_unicast_link_local() // fe80::/10, mirroring the IPv4 link-local case
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
    pub fn connection_duration(&self) -> Option<Duration> {
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

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::AppConnectInfo;

    fn info(addr: &str) -> AppConnectInfo {
        AppConnectInfo::new(addr.parse::<SocketAddr>().unwrap())
    }

    #[test]
    fn classifies_private_and_public_ipv4() {
        for private in [
            "10.0.0.1:80",
            "172.16.5.4:80",
            "192.168.1.1:80",
            "127.0.0.1:80",
            "169.254.1.1:80",
        ] {
            assert!(info(private).is_private_ip(), "{private} should be private");
            assert!(!info(private).is_public_ip());
        }
        for public in ["8.8.8.8:80", "1.1.1.1:443"] {
            assert!(info(public).is_public_ip(), "{public} should be public");
            assert!(!info(public).is_private_ip());
        }
    }

    #[test]
    fn classifies_ipv6_including_link_local() {
        // Loopback, unique-local (fc00::/7), and — the case the old bit-check
        // missed — link-local (fe80::/10) are all private.
        for private in [
            "[::1]:80",
            "[fc00::1]:80",
            "[fd12:3456::1]:80",
            "[fe80::1]:80",
        ] {
            assert!(info(private).is_private_ip(), "{private} should be private");
        }
        // A global-unicast address is public.
        assert!(info("[2606:4700::1111]:443").is_public_ip());
    }

    #[test]
    fn localhost_and_ip_family_predicates() {
        assert!(info("127.0.0.1:80").is_localhost());
        assert!(info("[::1]:80").is_localhost());
        assert!(!info("8.8.8.8:80").is_localhost());

        assert!(info("8.8.8.8:80").is_ipv4());
        assert!(!info("8.8.8.8:80").is_ipv6());
        assert!(info("[::1]:80").is_ipv6());
    }

    #[test]
    fn client_ip_prefers_the_proxy_real_ip() {
        let proxy: SocketAddr = "10.0.0.9:1234".parse().unwrap();
        let real = "203.0.113.7".parse().unwrap();
        let info = AppConnectInfo::with_real_ip(proxy, real);

        // The real (client) IP wins over the direct proxy address.
        assert_eq!(info.client_ip(), real);
        assert!(
            info.is_public_ip(),
            "classification follows the real client IP"
        );
        assert_eq!(info.client_port(), 1234);
        // The log string notes both the real IP and the proxy it came via.
        assert_eq!(info.to_log_string(), "203.0.113.7 (via 10.0.0.9)");
    }
}
