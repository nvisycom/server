//! Request structures for API token operations.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Expiration options for API tokens.
#[must_use]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TokenExpiration {
    /// Token never expires.
    Never,
    /// Expires in 7 days.
    #[default]
    In7Days,
    /// Expires in 30 days.
    In30Days,
    /// Expires in 90 days.
    In90Days,
    /// Expires in 1 year.
    In1Year,
}

impl TokenExpiration {
    /// Returns the duration until expiration, or None if never expires.
    pub fn to_span(self) -> Option<jiff::Span> {
        match self {
            Self::Never => None,
            Self::In7Days => Some(jiff::Span::new().days(7)),
            Self::In30Days => Some(jiff::Span::new().days(30)),
            Self::In90Days => Some(jiff::Span::new().days(90)),
            Self::In1Year => Some(jiff::Span::new().days(365)),
        }
    }

    /// Returns the expiry timestamp from now, or None if never expires.
    pub fn to_expiry_timestamp(self) -> Option<jiff::Timestamp> {
        self.to_span().map(|span| jiff::Timestamp::now() + span)
    }

    /// Returns the duration until expiration, or None if never expires.
    pub fn to_duration(self) -> Option<Duration> {
        match self {
            Self::Never => None,
            Self::In7Days => Some(Duration::from_secs(7 * 24 * 60 * 60)),
            Self::In30Days => Some(Duration::from_secs(30 * 24 * 60 * 60)),
            Self::In90Days => Some(Duration::from_secs(90 * 24 * 60 * 60)),
            Self::In1Year => Some(Duration::from_secs(365 * 24 * 60 * 60)),
        }
    }
}

/// Request to create a new API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiToken {
    /// Human-readable name for the API token.
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    /// Optional description for the API token.
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// When the token expires.
    #[serde(default)]
    pub expires: TokenExpiration,
}

/// Request to update an existing API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiToken {
    /// Updated name for the API token.
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,

    /// Updated description for the API token.
    #[validate(length(max = 500))]
    pub description: Option<String>,
}
