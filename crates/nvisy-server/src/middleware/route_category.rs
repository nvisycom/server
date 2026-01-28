//! Route categorization for metrics and logging.
//!
//! This module provides a categorization system for routes based on their
//! URI path, enabling aggregated metrics and monitoring by functional area.

use axum::http::Uri;

/// Route classification for metrics grouping.
///
/// Categorizes routes based on their URI path for aggregated metrics
/// and monitoring purposes. Each category represents a distinct
/// functional area of the API.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RouteCategory {
    /// Authentication routes (`/auth/*`).
    Authentication,
    /// Account management routes (`/accounts/*`).
    Accounts,
    /// Workspace management routes (`/workspaces/*`).
    Workspaces,
    /// Document routes (`/documents/*`).
    Documents,
    /// File operations routes (`/files/*`).
    Files,
    /// Connection routes (`/connections/*`).
    Connections,
    /// Webhook routes (`/webhooks/*`).
    Webhooks,
    /// Health and monitoring routes (`/monitors/*`).
    Monitors,
    /// API documentation routes (`/api/*`).
    Api,
    /// Unknown or uncategorized routes.
    Unknown,
}

impl RouteCategory {
    /// Categorizes a route based on its URI path.
    pub fn from_uri(uri: &Uri) -> Self {
        let path = uri.path();

        if path.starts_with("/auth/") {
            Self::Authentication
        } else if path.starts_with("/accounts/") {
            Self::Accounts
        } else if path.starts_with("/workspaces/") {
            Self::Workspaces
        } else if path.starts_with("/documents/") {
            Self::Documents
        } else if path.starts_with("/files/") {
            Self::Files
        } else if path.starts_with("/connections/") {
            Self::Connections
        } else if path.starts_with("/webhooks/") {
            Self::Webhooks
        } else if path.starts_with("/monitors/") {
            Self::Monitors
        } else if path.starts_with("/api/") {
            Self::Api
        } else {
            Self::Unknown
        }
    }

    /// Returns the string representation for logging and metrics.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authentication => "auth",
            Self::Accounts => "accounts",
            Self::Workspaces => "workspaces",
            Self::Documents => "documents",
            Self::Files => "files",
            Self::Connections => "connections",
            Self::Webhooks => "webhooks",
            Self::Monitors => "monitors",
            Self::Api => "api",
            Self::Unknown => "unknown",
        }
    }
}
