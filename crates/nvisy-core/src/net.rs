//! Endpoint policy and SSRF protection for caller-supplied URLs.
//!
//! Several connection kinds let a workspace member supply a custom endpoint URL
//! (object-store `endpoint`, an Ollama `base_url`), and the server then makes
//! authenticated requests to it. Left unchecked, that is a server-side request
//! forgery (SSRF) and cleartext-credential surface: a member could point the
//! server at loopback, a private/internal host, or the cloud metadata endpoint,
//! or send credentials over plaintext `http`.
//!
//! [`EndpointPolicy`] captures the deployment's stance:
//!
//! - [`Permissive`](EndpointPolicy::Permissive) — self-hosted, where endpoints
//!   are a trusted operator's choice: `https` anywhere, and `http` only for a
//!   loopback host (a local emulator such as MinIO, Azurite, or Ollama).
//! - [`Strict`](EndpointPolicy::Strict) — cloud/multi-tenant, where endpoints
//!   are attacker-influenced: `https` only, and the host must resolve entirely
//!   to globally routable addresses (checked after DNS, since a public hostname
//!   can resolve to a private address).
//!
//! `Strict` validation runs both when a connection is created/updated and again
//! at connect time, so a hostname that later starts resolving to an internal
//! address is caught before the next request. What remains is a narrow
//! DNS-rebinding race: the resolution this module checks and the one the backend
//! HTTP client performs a moment later are independent, so an attacker
//! controlling the hostname's DNS could, in principle, return a routable address
//! here and a private one to the client microseconds later. Closing that fully
//! needs the resolved IP pinned into the connection, which the object-store
//! client stack does not expose. The window is sub-second and hard to land; it
//! is an accepted residual limitation.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::error::{Error, ErrorKind, Result};

/// How the deployment treats caller-supplied endpoint URLs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EndpointPolicy {
    /// Self-hosted: `https` anywhere; `http` only for a loopback host.
    #[default]
    Permissive,
    /// Cloud/multi-tenant: `https` only, resolving to global addresses only.
    Strict,
}

/// The outcome of validating an endpoint under an [`EndpointPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointDecision {
    /// Whether the provider client should permit plaintext HTTP for this
    /// endpoint. Only ever `true` under [`Permissive`](EndpointPolicy::Permissive)
    /// for a loopback host.
    pub allow_http: bool,
}

impl EndpointPolicy {
    /// Validates a caller-supplied `endpoint` under this policy, returning
    /// whether the provider client may use plaintext HTTP for it.
    ///
    /// [`Strict`](Self::Strict) resolves the host and rejects the endpoint if any
    /// resolved address is non-routable, so it is async; [`Permissive`](Self::Permissive)
    /// does no DNS and completes immediately.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the URL is malformed, uses a disallowed scheme, or
    /// (under `Strict`) resolves to a non-routable address.
    pub async fn validate_endpoint(self, endpoint: &str) -> Result<EndpointDecision> {
        let url = Url::parse(endpoint)
            .map_err(|e| Error::invalid(format!("invalid endpoint URL: {e}")))?;
        match self {
            Self::Permissive => match url.scheme() {
                "https" => Ok(EndpointDecision { allow_http: false }),
                "http" if is_loopback_host(url.host()) => Ok(EndpointDecision { allow_http: true }),
                "http" => Err(Error::invalid(format!(
                    "plaintext http endpoints are only allowed for loopback hosts: {endpoint}"
                ))),
                other => Err(Error::invalid(format!(
                    "endpoint scheme must be http or https, got {other}: {endpoint}"
                ))),
            },
            Self::Strict => {
                if url.scheme() != "https" {
                    return Err(Error::invalid(format!(
                        "endpoint must use https: {endpoint}"
                    )));
                }
                let addrs = resolve_host(&url).await?;
                reject_non_global(&url, addrs)?;
                Ok(EndpointDecision { allow_http: false })
            }
        }
    }
}

/// Whether a parsed URL host is the local loopback interface: `localhost`, an
/// IPv4 loopback (`127.0.0.0/8`), or `::1`.
fn is_loopback_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// Resolves a URL's host to its socket addresses, using the explicit port or the
/// scheme default. The port does not affect address classification; it only
/// makes the resolver happy.
async fn resolve_host(url: &Url) -> Result<Vec<IpAddr>> {
    let host = url
        .host_str()
        .ok_or_else(|| Error::invalid(format!("endpoint has no host: {url}")))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| Error::connection(format!("could not resolve endpoint host {host}: {e}")))?
        .map(|socket| socket.ip())
        .collect();
    Ok(addrs)
}

/// Rejects the endpoint if the resolution is empty or any address is not a
/// globally routable unicast address.
fn reject_non_global(url: &Url, addrs: Vec<IpAddr>) -> Result<()> {
    let mut resolved = false;
    for addr in addrs {
        resolved = true;
        if !is_globally_routable(addr) {
            return Err(
                Error::new(ErrorKind::PermissionDenied).with_message(format!(
                    "endpoint host resolves to a non-routable address: {addr}"
                )),
            );
        }
    }
    if resolved {
        Ok(())
    } else {
        Err(Error::connection(format!(
            "endpoint host did not resolve: {url}"
        )))
    }
}

/// Whether an address is a globally routable unicast address, i.e. safe to reach
/// from the server.
///
/// The inverse blocks loopback, private ranges, link-local (which covers the
/// `169.254.169.254` cloud metadata endpoint), the unspecified address,
/// multicast, IPv6 unique-local, and the documentation/benchmark ranges. IPv6
/// forms that embed an IPv4 address are classified by that embedded address so
/// they cannot smuggle a blocked v4 target through.
#[must_use]
pub fn is_globally_routable(addr: IpAddr) -> bool {
    !is_blocked(addr)
}

fn is_blocked(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(ip) => is_blocked_v4(ip),
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

    #[tokio::test]
    async fn permissive_allows_https_anywhere() {
        let d = EndpointPolicy::Permissive
            .validate_endpoint("https://s3.example.com")
            .await
            .unwrap();
        assert!(!d.allow_http);
    }

    #[tokio::test]
    async fn permissive_allows_http_only_for_loopback() {
        for ep in [
            "http://localhost:9000",
            "http://localhost/devstore",
            "http://127.0.0.1:10000/x",
            "http://[::1]/path",
        ] {
            let d = EndpointPolicy::Permissive
                .validate_endpoint(ep)
                .await
                .unwrap();
            assert!(d.allow_http, "{ep}");
        }
    }

    #[tokio::test]
    async fn permissive_rejects_remote_http_and_bad_scheme() {
        for ep in ["http://s3.example.com", "ftp://host/x", "s3.example.com"] {
            assert!(
                EndpointPolicy::Permissive
                    .validate_endpoint(ep)
                    .await
                    .is_err(),
                "{ep}"
            );
        }
    }

    #[tokio::test]
    async fn strict_rejects_non_https() {
        for ep in ["http://localhost:9000", "http://s3.example.com"] {
            assert!(
                EndpointPolicy::Strict.validate_endpoint(ep).await.is_err(),
                "{ep}"
            );
        }
    }

    #[test]
    fn blocks_internal_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "192.168.1.1",
            "172.16.0.1",
            "169.254.169.254",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:169.254.169.254",
            "64:ff9b::a9fe:a9fe",
        ] {
            let addr: IpAddr = ip.parse().unwrap();
            assert!(!is_globally_routable(addr), "{ip}");
        }
    }

    #[test]
    fn allows_public_addresses() {
        for ip in ["93.184.216.34", "8.8.8.8", "2606:4700:4700::1111"] {
            let addr: IpAddr = ip.parse().unwrap();
            assert!(is_globally_routable(addr), "{ip}");
        }
    }
}
