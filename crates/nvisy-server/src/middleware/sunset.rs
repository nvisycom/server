//! Sunset deprecation header middleware.
//!
//! Adds `Sunset`, `Deprecation`, and `Link` HTTP headers to responses
//! from deprecated API versions, signalling to clients that the version
//! will be removed after a specified date.
//!
//! Headers follow [RFC 8594](https://httpwg.org/specs/rfc8594.html) and
//! the [Deprecation header draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-deprecation-header/).

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::Extension;
use axum::http::{HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;
use jiff::civil::Date;

use crate::extract::Version;

/// Per-version sunset entry: precomputed header values.
#[derive(Clone)]
struct SunsetEntry {
    sunset_date: HeaderValue,
    successor_link: HeaderValue,
}

/// Configuration for the sunset deprecation middleware.
///
/// Maps API version numbers to their sunset date. The successor
/// version is automatically set to `version + 1`. Only versions
/// present in the map receive deprecation headers; active versions
/// pass through unmodified.
///
/// Cloning is cheap: the inner map is behind an [`Arc`].
///
/// # Example
///
/// ```rust
/// use axum::middleware;
/// use jiff::civil::date;
/// use nvisy_server::middleware::SunsetConfig;
///
/// let config = SunsetConfig::new()
///     .deprecate(1, date(2025, 11, 1));
/// ```
#[derive(Clone, Default)]
pub struct SunsetConfig {
    versions: Arc<HashMap<u16, SunsetEntry>>,
}

impl SunsetConfig {
    /// Creates an empty config with no deprecated versions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a deprecated API version.
    ///
    /// - `version`: the version number (e.g. `1` for `/api/v1`)
    /// - `sunset_date`: the date after which the version may be removed
    ///
    /// The successor is automatically set to `version + 1`.
    ///
    /// # Panics
    ///
    /// Panics if `version` is 0.
    pub fn deprecate(mut self, version: u16, sunset_date: Date) -> Self {
        assert!(version > 0, "API version must be non-zero");

        let http_date = sunset_date
            .strftime("%a, %d %b %Y 00:00:00 GMT")
            .to_string();
        let successor = version + 1;

        Arc::make_mut(&mut self.versions).insert(
            version,
            SunsetEntry {
                sunset_date: HeaderValue::from_str(&http_date)
                    .expect("formatted date must be a valid header value"),
                successor_link: HeaderValue::from_str(&format!(
                    "</api/v{successor}>; rel=\"successor-version\""
                ))
                .expect("successor link must be a valid header value"),
            },
        );
        self
    }
}

/// Axum middleware function that adds sunset deprecation headers to
/// responses for deprecated API versions.
///
/// Extracts the version number from the [`Version`] extractor and
/// checks it against the configured deprecated versions. Requests
/// that don't match any deprecated version pass through unmodified.
pub async fn sunset_headers(
    Extension(config): Extension<SunsetConfig>,
    version: Version,
    req: Request<Body>,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;

    if let Some(entry) = version.into_inner().and_then(|v| config.versions.get(&v)) {
        let headers = response.headers_mut();
        headers.insert("sunset", entry.sunset_date.clone());
        headers.insert("deprecation", HeaderValue::from_static("true"));
        headers.append("link", entry.successor_link.clone());
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deprecate_builds_headers() {
        let config = SunsetConfig::new().deprecate(1, Date::new(2025, 11, 1).unwrap());
        assert!(config.versions.contains_key(&1));
        assert!(!config.versions.contains_key(&2));
    }

    #[test]
    fn successor_is_version_plus_one() {
        let config = SunsetConfig::new().deprecate(3, Date::new(2026, 6, 15).unwrap());
        let entry = &config.versions[&3];
        assert_eq!(
            entry.successor_link,
            HeaderValue::from_static("</api/v4>; rel=\"successor-version\""),
        );
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn deprecate_zero_panics() {
        SunsetConfig::new().deprecate(0, Date::new(2025, 1, 1).unwrap());
    }
}
