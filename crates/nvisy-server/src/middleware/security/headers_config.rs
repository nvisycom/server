//! Security headers configuration.

use serde::{Deserialize, Serialize};

/// Security headers configuration for the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct SecurityHeadersConfig {
    /// HTTP Strict Transport Security (HSTS) max age in seconds.
    /// Forces browsers to use HTTPS for the specified duration.
    /// Default: 1 year (31536000 seconds)
    pub hsts_max_age_seconds: u64,

    /// Whether to include subdomains in HSTS policy
    pub hsts_include_subdomains: bool,

    /// Content Security Policy (CSP) directives.
    /// Controls which resources the browser is allowed to load.
    pub content_security_policy: Option<String>,

    /// X-Frame-Options header value.
    /// Protects against clickjacking attacks.
    pub frame_options: FrameOptions,

    /// Referrer-Policy header value.
    /// Controls how much referrer information is included with requests.
    pub referrer_policy: ReferrerPolicy,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            hsts_max_age_seconds: 31_536_000, // 1 year
            hsts_include_subdomains: true,
            content_security_policy: Some(
                "default-src 'self'; \
                 script-src 'self' 'unsafe-inline'; \
                 style-src 'self' 'unsafe-inline'; \
                 img-src 'self' data:; \
                 connect-src 'self'; \
                 frame-ancestors 'none'; \
                 base-uri 'self'; \
                 form-action 'self'"
                    .to_string(),
            ),
            frame_options: FrameOptions::Deny,
            referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
        }
    }
}

impl SecurityHeadersConfig {
    /// Returns the HSTS header value as a string.
    pub fn hsts_header_value(&self) -> String {
        if self.hsts_include_subdomains {
            format!("max-age={}; includeSubDomains", self.hsts_max_age_seconds)
        } else {
            format!("max-age={}", self.hsts_max_age_seconds)
        }
    }

    /// Returns the CSP header value if configured.
    pub fn csp_header_value(&self) -> Option<&str> {
        self.content_security_policy.as_deref()
    }

    /// Returns the Frame-Options header value.
    pub fn frame_options_value(&self) -> &'static str {
        self.frame_options.as_str()
    }

    /// Returns the Referrer-Policy header value.
    pub fn referrer_policy_value(&self) -> &'static str {
        self.referrer_policy.as_str()
    }
}

/// X-Frame-Options header values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameOptions {
    /// The page cannot be displayed in a frame, regardless of the site attempting to do so.
    Deny,
    /// The page can only be displayed in a frame on the same origin as the page itself.
    SameOrigin,
}

impl FrameOptions {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "DENY",
            Self::SameOrigin => "SAMEORIGIN",
        }
    }
}

/// Referrer-Policy header values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferrerPolicy {
    /// No referrer information is sent.
    NoReferrer,
    /// Sends only the origin (scheme, host, and port) as the referrer.
    Origin,
    /// Sends the full URL when performing a same-origin request, but only the origin for cross-origin requests.
    StrictOriginWhenCrossOrigin,
}

impl ReferrerPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoReferrer => "no-referrer",
            Self::Origin => "origin",
            Self::StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsts_header_value() {
        let config = SecurityHeadersConfig {
            hsts_max_age_seconds: 31536000,
            hsts_include_subdomains: true,
            ..Default::default()
        };
        assert_eq!(
            config.hsts_header_value(),
            "max-age=31536000; includeSubDomains"
        );

        let config_no_subdomains = SecurityHeadersConfig {
            hsts_max_age_seconds: 31536000,
            hsts_include_subdomains: false,
            ..Default::default()
        };
        assert_eq!(config_no_subdomains.hsts_header_value(), "max-age=31536000");
    }

    #[test]
    fn test_frame_options() {
        assert_eq!(FrameOptions::Deny.as_str(), "DENY");
        assert_eq!(FrameOptions::SameOrigin.as_str(), "SAMEORIGIN");
    }

    #[test]
    fn test_referrer_policy() {
        assert_eq!(ReferrerPolicy::NoReferrer.as_str(), "no-referrer");
        assert_eq!(ReferrerPolicy::Origin.as_str(), "origin");
        assert_eq!(
            ReferrerPolicy::StrictOriginWhenCrossOrigin.as_str(),
            "strict-origin-when-cross-origin"
        );
    }
}
