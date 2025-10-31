use std::convert::Infallible;
use std::fmt;
use std::num::NonZeroU32;

use axum::RequestPartsExt;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde::Deserialize;

use crate::extract::Path;

/// Version prefix used in version strings (e.g., "v1", "v2").
const VERSION_PREFIX: char = 'v';

/// The unstable version number.
const UNSTABLE_VERSION: u32 = 0;

/// Enhanced version parameter extractor for API versioning support.
///
/// This extractor handles API version parameters from URL paths, providing
/// robust parsing and validation of version strings. It supports:
/// - Stable versions: `v1`, `v2`, `v3`, etc.
/// - Unstable version: `v0` (for development/experimental endpoints)
/// - Graceful handling of invalid or unrecognized version formats
///
/// # Supported Version Formats
///
/// - **Stable versions**: `v{positive_integer}` (e.g., `v1`, `v2`, `v10`)
/// - **Unstable version**: `v0` (experimental/development endpoints)
/// - **Invalid formats**: Any string not matching the above patterns
///
/// # Version Validation
///
/// The extractor automatically parses and validates version parameters:
/// - `v1` → `Version::Stable(NonZeroU32::new(1).unwrap())`
/// - `v0` → `Version::Unstable`
/// - `invalid` → `Version::Unrecognized`
#[must_use]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// Represents an invalid or unrecognized version format.
    ///
    /// This variant is used when the version parameter doesn't match
    /// the expected `v{number}` pattern or contains invalid characters.
    #[default]
    Unrecognized,

    /// Represents the unstable development version (v0).
    ///
    /// This version is typically used for experimental features
    /// and development endpoints that may change frequently.
    Unstable,

    /// Represents a stable API version (v1, v2, v3, etc.).
    ///
    /// These versions follow semantic versioning principles and
    /// are expected to provide backward compatibility guarantees.
    /// The contained `NonZeroU32` represents the version number.
    Stable(NonZeroU32),
}

impl Version {
    /// Creates a new [`Version`] by parsing a version string.
    ///
    /// # Arguments
    ///
    /// * `version` - A version string in the format "v{number}" (e.g., "v1", "v2", "v0")
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::num::NonZeroU32;
    /// # use nvisy_server::extract::Version;
    /// assert_eq!(Version::new("v1"), Version::Stable(NonZeroU32::new(1).unwrap()));
    /// assert_eq!(Version::new("v0"), Version::Unstable);
    /// assert_eq!(Version::new("invalid"), Version::Unrecognized);
    /// ```
    ///
    /// # Returns
    ///
    /// - `Version::Stable(n)` for valid positive integers (v1, v2, etc.)
    /// - `Version::Unstable` for v0
    /// - `Version::Unrecognized` for invalid formats
    pub fn new(version: &str) -> Self {
        let number = version
            .strip_prefix(VERSION_PREFIX)
            .and_then(|x| x.parse::<u32>().ok());

        match number.map(NonZeroU32::new) {
            None => Self::Unrecognized,
            Some(Some(x)) => Self::Stable(x),
            Some(None) => Self::Unstable,
        }
    }

    /// Returns `true` if this represents an unrecognized or invalid version.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_server::extract::Version;
    /// assert!(Version::new("invalid").is_unrecognized());
    /// assert!(!Version::new("v1").is_unrecognized());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_unrecognized(&self) -> bool {
        matches!(self, Self::Unrecognized)
    }

    /// Returns `true` if this represents the unstable version (v0).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_server::extract::Version;
    /// assert!(Version::new("v0").is_unstable());
    /// assert!(!Version::new("v1").is_unstable());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_unstable(&self) -> bool {
        matches!(self, Self::Unstable)
    }

    /// Returns `true` if this represents a stable version (v1, v2, etc.).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_server::extract::Version;
    /// assert!(Version::new("v1").is_stable());
    /// assert!(Version::new("v2").is_stable());
    /// assert!(!Version::new("v0").is_stable());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_stable(&self) -> bool {
        matches!(self, Self::Stable(_))
    }

    /// Returns `true` if this version matches the specified version number.
    ///
    /// # Arguments
    ///
    /// * `version` - The version number to check against (0 for unstable, 1+ for stable)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_server::extract::Version;
    /// let v1 = Version::new("v1");
    /// let v0 = Version::new("v0");
    ///
    /// assert!(v1.is_v(1));
    /// assert!(!v1.is_v(2));
    /// assert!(v0.is_v(0));
    /// assert!(!v0.is_v(1));
    /// ```
    #[must_use]
    pub fn is_v(&self, version: u32) -> bool {
        match self {
            Self::Unstable => version == UNSTABLE_VERSION,
            Self::Stable(x) => x.get() == version,
            Self::Unrecognized => false,
        }
    }

    /// Returns the underlying version number, if available.
    ///
    /// # Returns
    ///
    /// * `Some(0)` for unstable versions
    /// * `Some(n)` for stable versions where n > 0
    /// * `None` for unrecognized versions
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_server::extract::Version;
    /// assert_eq!(Version::new("v1").into_inner(), Some(1));
    /// assert_eq!(Version::new("v0").into_inner(), Some(0));
    /// assert_eq!(Version::new("invalid").into_inner(), None);
    /// ```
    pub fn into_inner(self) -> Option<u32> {
        match self {
            Self::Unrecognized => None,
            Self::Stable(x) => Some(x.get()),
            Self::Unstable => Some(UNSTABLE_VERSION),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unrecognized => write!(f, "unrecognized"),
            Self::Unstable => write!(f, "unstable"),
            Self::Stable(x) => write!(f, "v{}", x.get()),
        }
    }
}

impl From<Version> for bool {
    #[inline]
    fn from(value: Version) -> Self {
        value.is_stable()
    }
}

impl<S> FromRequestParts<S> for Version
where
    S: Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        #[derive(Debug, Clone, Deserialize)]
        struct VersionParams {
            pub version: String,
        }

        Ok(match parts.extract::<Path<VersionParams>>().await {
            Ok(params) => Version::new(&params.version),
            Err(_) => Version::default(),
        })
    }
}
