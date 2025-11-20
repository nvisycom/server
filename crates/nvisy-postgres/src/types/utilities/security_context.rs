//! Security-related helper utilities and traits for models with security context.
//!
//! This module provides security analysis capabilities for models that track
//! IP addresses, user agents, and geographic information.

use ipnet::IpNet;

/// Trait for models that track security context (IP address, user agent, device info).
pub trait HasSecurityContext {
    /// Returns the IP address if available.
    fn ip_address(&self) -> Option<IpNet>;

    /// Returns the user agent string if available.
    fn user_agent(&self) -> Option<&str>;

    /// Returns whether this represents a potentially suspicious activity.
    fn is_security_relevant(&self) -> bool {
        self.ip_address().is_some() || self.user_agent().is_some()
    }

    /// Returns whether the IP address appears to be from a private network.
    fn is_from_private_network(&self) -> bool {
        if let Some(ip) = self.ip_address() {
            match ip {
                IpNet::V4(net) => {
                    let addr = net.addr();
                    addr.is_private() || addr.is_loopback()
                }
                IpNet::V6(net) => {
                    let addr = net.addr();
                    addr.is_loopback()
                        || addr.to_string().starts_with("fc")
                        || addr.to_string().starts_with("fd")
                }
            }
        } else {
            false
        }
    }

    /// Returns a security summary for logging and analysis.
    fn security_summary(&self) -> String {
        match (self.ip_address(), self.user_agent()) {
            (Some(ip), Some(ua)) => format!("IP: {} | UA: {}", ip, ua),
            (Some(ip), None) => format!("IP: {} | UA: Not recorded", ip),
            (None, Some(ua)) => format!("IP: Not recorded | UA: {}", ua),
            (None, None) => "No security context recorded".to_string(),
        }
    }
}

/// Trait for models that have geographic location information.
pub trait HasGeographicContext {
    /// Returns the country code if available.
    fn country_code(&self) -> Option<&str>;

    /// Returns the region/state code if available.
    fn region_code(&self) -> Option<&str>;

    /// Returns the city name if available.
    fn city_name(&self) -> Option<&str>;

    /// Returns whether geographic information is available.
    fn has_geographic_info(&self) -> bool {
        self.country_code().is_some() || self.region_code().is_some() || self.city_name().is_some()
    }

    /// Returns a formatted location string for display.
    fn location_display(&self) -> String {
        let parts: Vec<&str> = [self.city_name(), self.region_code(), self.country_code()]
            .iter()
            .filter_map(|&opt| opt)
            .collect();

        if parts.is_empty() {
            "Unknown location".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Returns whether the location appears to be outside expected regions.
    fn is_unusual_location(&self, expected_countries: &[&str]) -> bool {
        if let Some(country) = self.country_code() {
            !expected_countries.contains(&country)
        } else {
            true // No location data is unusual
        }
    }
}
