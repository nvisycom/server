//! Regional data collection policy management.
//!
//! This module provides types and utilities for managing data collection policies
//! based on regional compliance requirements such as GDPR, CCPA, and other
//! privacy regulations.

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Regional data collection policy defining how user data should be handled.
///
/// This enum represents different levels of data collection and processing
/// based on regional privacy regulations and user preferences.
///
/// # Examples
///
/// ```rust
/// use nvisy_server::service::policy::RegionalPolicy;
///
/// // Create policies
/// let minimal = RegionalPolicy::minimal();
/// let normal = RegionalPolicy::normal();
///
/// // Check policy type
/// assert!(minimal.is_minimal());
/// assert!(normal.is_normal());
///
/// // Convert from boolean (legacy support)
/// let policy = RegionalPolicy::from_minimal_flag(true);
/// assert_eq!(policy, RegionalPolicy::MinimalDataCollection);
/// ```
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
)]
#[derive(
    Serialize,
    Deserialize,
    ToSchema
)]
#[serde(rename_all = "lowercase")]
pub enum RegionalPolicy {
    /// Minimal data collection policy.
    ///
    /// This policy is designed for regions with strict privacy regulations
    /// such as the EU (GDPR) or California (CCPA). It:
    /// - Collects only essential data required for core functionality
    /// - Minimizes data retention periods
    /// - Requires explicit consent for any non-essential data processing
    /// - Provides enhanced user control over data handling
    #[default]
    #[serde(rename = "minimal")]
    MinimalDataCollection,

    /// Normal data collection policy.
    ///
    /// This policy allows standard data collection practices while still
    /// maintaining user privacy and security. It:
    /// - Collects data necessary for service improvement
    /// - Allows analytics and performance monitoring
    /// - Enables personalized user experiences
    /// - Follows industry-standard privacy practices
    #[serde(rename = "normal")]
    NormalDataCollection,
}

impl RegionalPolicy {
    /// Creates a new minimal data collection policy.
    ///
    /// This is equivalent to `RegionalPolicy::MinimalDataCollection` but
    /// provides a more ergonomic API.
    #[inline]
    pub const fn minimal() -> Self {
        Self::MinimalDataCollection
    }

    /// Creates a new normal data collection policy.
    ///
    /// This is equivalent to `RegionalPolicy::NormalDataCollection` but
    /// provides a more ergonomic API.
    #[inline]
    pub const fn normal() -> Self {
        Self::NormalDataCollection
    }

    /// Returns `true` if this is the minimal data collection policy.
    #[inline]
    pub const fn is_minimal(self) -> bool {
        matches!(self, Self::MinimalDataCollection)
    }

    /// Returns `true` if this is the normal data collection policy.
    #[inline]
    pub const fn is_normal(self) -> bool {
        matches!(self, Self::NormalDataCollection)
    }

    /// Returns the string representation used in serialization.
    ///
    /// This matches the serde rename values for consistency.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MinimalDataCollection => "minimal",
            Self::NormalDataCollection => "normal",
        }
    }

    /// Returns whether this policy requires explicit consent for data processing.
    #[inline]
    pub const fn requires_explicit_consent(self) -> bool {
        matches!(self, Self::MinimalDataCollection)
    }

    /// Returns whether this policy allows analytics data collection.
    #[inline]
    pub const fn allows_analytics(self) -> bool {
        matches!(self, Self::NormalDataCollection)
    }

    /// Returns the maximum data retention period in days for this policy.
    ///
    /// This provides guidance for data retention policies based on the
    /// collection level.
    pub const fn max_retention_days(self) -> u32 {
        match self {
            // 1 year - conservative
            Self::MinimalDataCollection => 365,
            // 3 years - standard
            Self::NormalDataCollection => 1095,
        }
    }
}

impl fmt::Display for RegionalPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(
            RegionalPolicy::default(),
            RegionalPolicy::MinimalDataCollection
        );
    }

    #[test]
    fn test_predicates() {
        let minimal = RegionalPolicy::MinimalDataCollection;
        let normal = RegionalPolicy::NormalDataCollection;

        assert!(minimal.is_minimal());
        assert!(!minimal.is_normal());

        assert!(!normal.is_minimal());
        assert!(normal.is_normal());
    }

    #[test]
    fn test_string_representation() {
        assert_eq!(RegionalPolicy::MinimalDataCollection.as_str(), "minimal");
        assert_eq!(RegionalPolicy::NormalDataCollection.as_str(), "normal");
    }

    #[test]
    fn test_policy_properties() {
        let minimal = RegionalPolicy::MinimalDataCollection;
        let normal = RegionalPolicy::NormalDataCollection;

        // Test consent requirements
        assert!(minimal.requires_explicit_consent());
        assert!(!normal.requires_explicit_consent());

        // Test analytics
        assert!(!minimal.allows_analytics());
        assert!(normal.allows_analytics());

        // Test retention periods
        assert!(minimal.max_retention_days() < normal.max_retention_days());
        assert_eq!(minimal.max_retention_days(), 365);
        assert_eq!(normal.max_retention_days(), 1095);
    }
}
