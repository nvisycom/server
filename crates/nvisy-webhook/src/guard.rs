//! SSRF protection for webhook delivery URLs.
//!
//! Webhook endpoints are user-supplied, so a delivery must never reach an
//! internal address (loopback, private ranges, link-local, the cloud metadata
//! endpoint, and so on). The `UrlGuardExt` trait extends `Url` with two checks:
//!
//! - `check_scheme` rejects anything that is not `http`/`https`. It is cheap and
//!   runs at create/update time for fast feedback.
//! - `check_resolved_addrs` rejects a delivery when any address the host
//!   resolves to is non-global. It runs at delivery time, after DNS resolution,
//!   because a hostname can resolve to a private address.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::Url;

use crate::{Error, ErrorKind, Result};

/// Extends `Url` with SSRF checks for webhook delivery.
pub trait UrlGuardExt {
    /// Returns an `InvalidEndpoint` error unless the URL uses the `http` or
    /// `https` scheme.
    fn check_scheme(&self) -> Result<()>;

    /// Returns an `InvalidEndpoint` error if the URL's host is an IP literal
    /// that is not globally routable.
    ///
    /// This is a synchronous, no-DNS check for write-time validation; hostnames
    /// pass here and are checked against their resolved addresses at delivery.
    fn check_literal_host(&self) -> Result<()>;

    /// Returns an `InvalidEndpoint` error if any resolved address is not a
    /// globally routable unicast address.
    ///
    /// `addrs` are the addresses the host resolved to. Rejecting when the list
    /// is empty prevents delivering to a host that resolved to nothing.
    fn check_resolved_addrs(&self, addrs: impl IntoIterator<Item = IpAddr>) -> Result<()>;
}

impl UrlGuardExt for Url {
    fn check_scheme(&self) -> Result<()> {
        match self.scheme() {
            "http" | "https" => Ok(()),
            other => Err(Error::new(ErrorKind::InvalidEndpoint)
                .with_message(format!("unsupported webhook URL scheme: {other}"))),
        }
    }

    fn check_literal_host(&self) -> Result<()> {
        match self.host() {
            Some(url::Host::Ipv4(ip)) if is_blocked(IpAddr::V4(ip)) => {
                Err(Error::new(ErrorKind::InvalidEndpoint)
                    .with_message(format!("webhook host is a non-routable address: {ip}")))
            }
            Some(url::Host::Ipv6(ip)) if is_blocked(IpAddr::V6(ip)) => {
                Err(Error::new(ErrorKind::InvalidEndpoint)
                    .with_message(format!("webhook host is a non-routable address: {ip}")))
            }
            _ => Ok(()),
        }
    }

    fn check_resolved_addrs(&self, addrs: impl IntoIterator<Item = IpAddr>) -> Result<()> {
        let mut resolved = false;
        for addr in addrs {
            resolved = true;
            if is_blocked(addr) {
                return Err(Error::new(ErrorKind::InvalidEndpoint).with_message(format!(
                    "webhook host resolves to a non-routable address: {addr}"
                )));
            }
        }

        if resolved {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::InvalidEndpoint)
                .with_message(format!("webhook host did not resolve: {self}")))
        }
    }
}

/// Returns whether an address must not be reached by a webhook delivery.
///
/// Blocks everything that is not a globally routable unicast address:
/// loopback, private ranges, link-local (which covers the `169.254.169.254`
/// cloud metadata endpoint), the unspecified address, multicast, and IPv6
/// unique-local addresses. IPv6 forms that embed an IPv4 address are classified
/// by that embedded address so they cannot smuggle a blocked v4 target through.
fn is_blocked(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(ip) => is_blocked_v4(ip),
        // Classify any embedded IPv4 (mapped ::ffff:0:0/96, NAT64 64:ff9b::/96,
        // deprecated compatible ::/96) by its v4 address rather than treating it
        // as a global v6 address.
        IpAddr::V6(ip) => match embedded_ipv4(ip) {
            Some(v4) => is_blocked_v4(v4),
            None => is_blocked_v6(ip),
        },
    }
}

/// Extracts an embedded IPv4 address from the IPv6 forms that carry one.
fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    // NAT64 well-known prefix, 64:ff9b::/96.
    if segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2..6] == [0, 0, 0, 0] {
        let [.., a, b, c, d] = ip.octets();
        return Some(Ipv4Addr::new(a, b, c, d));
    }
    // IPv4-mapped (::ffff:a.b.c.d) and deprecated IPv4-compatible (::a.b.c.d).
    ip.to_ipv4_mapped().or_else(|| match ip.to_ipv4() {
        Some(v4) if !ip.is_loopback() && !ip.is_unspecified() => Some(v4),
        _ => None,
    })
}

/// Returns whether an IPv6 address must not be reached by a webhook delivery.
fn is_blocked_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        // Unique-local addresses, fc00::/7.
        || (first & 0xfe00) == 0xfc00
        // Link-local unicast, fe80::/10.
        || (first & 0xffc0) == 0xfe80
        // Documentation range, 2001:db8::/32.
        || (first == 0x2001 && ip.segments()[1] == 0x0db8)
}

/// Returns whether an IPv4 address must not be reached by a webhook delivery.
fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        // Shared address space (carrier-grade NAT), 100.64.0.0/10.
        || (a == 100 && (64..128).contains(&b))
        // Benchmarking, 198.18.0.0/15.
        || (a == 198 && (b == 18 || b == 19))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        assert!(
            Url::parse("ftp://example.com")
                .unwrap()
                .check_scheme()
                .is_err()
        );
        assert!(
            Url::parse("file:///etc/passwd")
                .unwrap()
                .check_scheme()
                .is_err()
        );
        assert!(
            Url::parse("https://example.com")
                .unwrap()
                .check_scheme()
                .is_ok()
        );
        assert!(
            Url::parse("http://example.com")
                .unwrap()
                .check_scheme()
                .is_ok()
        );
    }

    #[test]
    fn blocks_metadata_endpoint() {
        assert!(is_blocked(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
    }

    #[test]
    fn blocks_private_and_loopback() {
        assert!(is_blocked(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_blocked(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_blocked(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_blocked(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_blocked(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
        assert!(is_blocked(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn allows_public_addresses() {
        assert!(!is_blocked(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))));
        assert!(!is_blocked(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }

    #[test]
    fn blocks_ipv6_embedded_ipv4_metadata() {
        // IPv4-mapped ::ffff:169.254.169.254
        assert!(is_blocked("::ffff:169.254.169.254".parse().unwrap()));
        // NAT64 64:ff9b::169.254.169.254
        assert!(is_blocked("64:ff9b::a9fe:a9fe".parse().unwrap()));
        // IPv4-compatible ::10.0.0.1
        assert!(is_blocked("::10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn blocks_ipv6_internal_ranges() {
        assert!(is_blocked("::1".parse().unwrap()));
        assert!(is_blocked("fc00::1".parse().unwrap()));
        assert!(is_blocked("fe80::1".parse().unwrap()));
        assert!(is_blocked("2001:db8::1".parse().unwrap()));
    }

    #[test]
    fn allows_public_ipv6() {
        // NAT64-wrapped public address stays allowed.
        assert!(!is_blocked("64:ff9b::5db8:d822".parse().unwrap()));
        assert!(!is_blocked("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn empty_resolution_is_rejected() {
        let url = Url::parse("https://example.com").unwrap();
        assert!(url.check_resolved_addrs(std::iter::empty()).is_err());
    }

    #[test]
    fn blocked_address_in_resolution_is_rejected() {
        let url = Url::parse("https://example.com").unwrap();
        let addrs = [
            IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        ];
        assert!(url.check_resolved_addrs(addrs).is_err());
    }

    #[test]
    fn all_public_resolution_is_allowed() {
        let url = Url::parse("https://example.com").unwrap();
        let addrs = [IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))];
        assert!(url.check_resolved_addrs(addrs).is_ok());
    }
}
