//! Utility modules for common functionality across the crate.

pub mod constants;
pub mod route_category;
pub mod tracing_targets;

use std::net::Ipv4Addr;

use ipnet::{IpNet, Ipv4Net};
pub use route_category::RouteCategory;

/// Returns a placeholder IP address (`0.0.0.0/32`) for use when client IP is unavailable.
///
/// TODO: Replace with real client IP extraction once `ClientIp` extractor is properly configured.
#[inline]
pub fn placeholder_ip() -> IpNet {
    IpNet::V4(Ipv4Net::new(Ipv4Addr::UNSPECIFIED, 32).unwrap())
}
