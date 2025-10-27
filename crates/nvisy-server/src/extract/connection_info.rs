use std::net::{IpAddr, SocketAddr};
use std::time::SystemTime;

use axum::extract::connect_info::Connected;
use axum::serve::IncomingStream;
use tokio::net::TcpListener;

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
/// # Examples
///
/// ```rust,no_run
/// use axum::extract::ConnectInfo;
/// use nvisy_server::extract::AppConnectInfo;
///
/// async fn handler(ConnectInfo(conn): ConnectInfo<AppConnectInfo>) {
///     println!("Client IP: {}", conn.client_ip());
///     println!("Is private IP: {}", conn.is_private_ip());
///     println!("Connection time: {:?}", conn.connected_at);
/// }
/// ```
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

#[cfg(test)]
mod test {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::str::FromStr;

    use axum::extract::ConnectInfo;
    use axum::routing::{Router, any};
    use axum_test::TestServer;

    use super::AppConnectInfo;
    use crate::handler::Result;

    async fn handler(ConnectInfo(conn): ConnectInfo<AppConnectInfo>) -> Result<String> {
        Ok(format!("Connected from: {}", conn.to_log_string()))
    }

    #[tokio::test]
    async fn extract_connection_info() -> anyhow::Result<()> {
        let router = Router::new().route("/", any(handler));
        let app = router.into_make_service_with_connect_info::<AppConnectInfo>();
        let server = TestServer::new(app)?;

        let response = server.get("/").await;
        assert!(response.text().contains("Connected from:"));

        Ok(())
    }

    #[test]
    fn test_private_ip_detection() {
        // Test private IPv4 addresses
        let private_ipv4 = AppConnectInfo::new(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            80,
        ));
        assert!(private_ipv4.is_private_ip());
        assert!(!private_ipv4.is_public_ip());

        // Test public IPv4 address
        let public_ipv4 =
            AppConnectInfo::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 80));
        assert!(public_ipv4.is_public_ip());
        assert!(!public_ipv4.is_private_ip());

        // Test loopback
        let loopback = AppConnectInfo::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80));
        assert!(loopback.is_localhost());
        assert!(loopback.is_private_ip());
    }

    #[test]
    fn test_ipv6_addresses() {
        let ipv6_loopback =
            AppConnectInfo::new(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 80));
        assert!(ipv6_loopback.is_ipv6());
        assert!(ipv6_loopback.is_localhost());
        assert!(ipv6_loopback.is_private_ip());

        // Test public IPv6
        let ipv6_public = AppConnectInfo::new(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap()),
            80,
        ));
        assert!(ipv6_public.is_ipv6());
        assert!(!ipv6_public.is_localhost());
    }

    #[test]
    fn test_real_ip_override() {
        let proxy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 80);
        let real_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1));

        let conn_info = AppConnectInfo::with_real_ip(proxy_addr, real_ip);

        assert_eq!(conn_info.addr.ip(), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(conn_info.client_ip(), real_ip);
        assert_eq!(conn_info.real_ip, Some(real_ip));
        assert!(conn_info.is_public_ip()); // Based on real IP

        let log_string = conn_info.to_log_string();
        assert!(log_string.contains("203.0.113.1"));
        assert!(log_string.contains("10.0.0.1"));
    }

    #[test]
    fn test_connection_duration() {
        let conn_info = AppConnectInfo::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80));

        // Duration should be very small but present
        let duration = conn_info.connection_duration();
        assert!(duration.is_some());
        assert!(duration.unwrap().as_millis() < 100); // Should be very recent
    }

    #[test]
    fn test_utility_methods() {
        let conn_info = AppConnectInfo::new(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            8080,
        ));

        assert_eq!(conn_info.client_port(), 8080);
        assert!(conn_info.is_ipv4());
        assert!(!conn_info.is_ipv6());

        // Test log string format
        let log_string = conn_info.to_log_string();
        assert_eq!(log_string, "192.168.1.100:8080");
    }
}
