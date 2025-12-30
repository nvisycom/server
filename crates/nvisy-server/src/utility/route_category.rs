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
    Authentication,
    UserManagement,
    WorkspaceManagement,
    DocumentProcessing,
    FileOperations,
    Automation,
    Support,
    Monitoring,
    Api,
    Unknown,
}

impl RouteCategory {
    /// Categorizes a route based on its URI path.
    pub fn from_uri(uri: &Uri) -> Self {
        let path = uri.path();

        if path.starts_with("/auth/") {
            Self::Authentication
        } else if path.starts_with("/accounts/") {
            Self::UserManagement
        } else if path.starts_with("/workspaces/") {
            Self::WorkspaceManagement
        } else if path.contains("/documents/") {
            Self::DocumentProcessing
        } else if path.contains("/inputs/") || path.contains("/outputs/") {
            Self::FileOperations
        } else if path.starts_with("/automation/") {
            Self::Automation
        } else if path.starts_with("/support/") || path.starts_with("/feedback/") {
            Self::Support
        } else if path.starts_with("/monitors/") {
            Self::Monitoring
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
            Self::UserManagement => "users",
            Self::WorkspaceManagement => "workspaces",
            Self::DocumentProcessing => "documents",
            Self::FileOperations => "files",
            Self::Automation => "automation",
            Self::Support => "support",
            Self::Monitoring => "monitoring",
            Self::Api => "api",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorization_maps_paths_correctly() {
        assert_eq!(
            RouteCategory::from_uri(&"/auth/login".parse().unwrap()),
            RouteCategory::Authentication
        );
        assert_eq!(
            RouteCategory::from_uri(&"/accounts/123".parse().unwrap()),
            RouteCategory::UserManagement
        );
        assert_eq!(
            RouteCategory::from_uri(&"/workspaces/456".parse().unwrap()),
            RouteCategory::WorkspaceManagement
        );
        assert_eq!(
            RouteCategory::from_uri(&"/workspaces/456/documents/789".parse().unwrap()),
            RouteCategory::WorkspaceManagement
        );
        assert_eq!(
            RouteCategory::from_uri(&"/documents/789/inputs/".parse().unwrap()),
            RouteCategory::DocumentProcessing
        );
        assert_eq!(
            RouteCategory::from_uri(&"/automation/batch".parse().unwrap()),
            RouteCategory::Automation
        );
        assert_eq!(
            RouteCategory::from_uri(&"/support/tickets".parse().unwrap()),
            RouteCategory::Support
        );
        assert_eq!(
            RouteCategory::from_uri(&"/monitors/health".parse().unwrap()),
            RouteCategory::Monitoring
        );
        assert_eq!(
            RouteCategory::from_uri(&"/api/swagger".parse().unwrap()),
            RouteCategory::Api
        );
        assert_eq!(
            RouteCategory::from_uri(&"/unknown/path".parse().unwrap()),
            RouteCategory::Unknown
        );
    }
}
