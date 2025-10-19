//! Request metrics and performance monitoring middleware.

use std::time::Instant;

use axum::extract::{ConnectInfo, Request};
use axum::http::Uri;
use axum::middleware::Next;
use axum::response::Response;
use tracing::{error, info, warn};

use crate::extract::AppConnectInfo;

/// Request classification for metrics grouping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequestCategory {
    Authentication,
    UserManagement,
    ProjectManagement,
    DocumentProcessing,
    FileOperations,
    Automation,
    Support,
    Monitoring,
    Api,
    Unknown,
}

impl RequestCategory {
    /// Categorize a request based on its URI path.
    pub fn from_uri(uri: &Uri) -> Self {
        let path = uri.path();

        if path.starts_with("/auth/") {
            Self::Authentication
        } else if path.starts_with("/accounts/") {
            Self::UserManagement
        } else if path.starts_with("/projects/") {
            Self::ProjectManagement
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

    /// Get the string representation for logging.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authentication => "auth",
            Self::UserManagement => "users",
            Self::ProjectManagement => "projects",
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

/// Performance thresholds for different request categories.
pub struct PerformanceThresholds {
    pub warn_ms: u64,
    pub error_ms: u64,
}

impl PerformanceThresholds {
    /// Get performance thresholds for a request category.
    pub fn for_category(category: &RequestCategory) -> Self {
        match category {
            RequestCategory::Authentication => Self {
                warn_ms: 500,
                error_ms: 2000,
            },
            RequestCategory::UserManagement => Self {
                warn_ms: 300,
                error_ms: 1000,
            },
            RequestCategory::ProjectManagement => Self {
                warn_ms: 400,
                error_ms: 1500,
            },
            RequestCategory::DocumentProcessing => Self {
                warn_ms: 2000,
                error_ms: 10000,
            },
            RequestCategory::FileOperations => Self {
                warn_ms: 5000,
                error_ms: 30000,
            },
            RequestCategory::Automation => Self {
                warn_ms: 10000,
                error_ms: 60000,
            },
            RequestCategory::Support => Self {
                warn_ms: 1000,
                error_ms: 5000,
            },
            RequestCategory::Monitoring => Self {
                warn_ms: 100,
                error_ms: 500,
            },
            RequestCategory::Api => Self {
                warn_ms: 200,
                error_ms: 1000,
            },
            RequestCategory::Unknown => Self {
                warn_ms: 1000,
                error_ms: 5000,
            },
        }
    }
}

/// Enhanced request metrics middleware with categorization.
pub async fn track_categorized_metrics(
    ConnectInfo(connect_info): ConnectInfo<AppConnectInfo>,
    request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let category = RequestCategory::from_uri(&uri);
    let client_ip = connect_info.addr.ip();

    let request_size = request
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    info!(
        target: "server::metrics::categorized",
        method = %method,
        uri = %uri,
        category = category.as_str(),
        client_ip = %client_ip,
        request_size = request_size,
        "Categorized request started"
    );

    let response = next.run(request).await;

    let duration = start_time.elapsed();
    let status = response.status();
    let thresholds = PerformanceThresholds::for_category(&category);

    let response_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let duration_ms = duration.as_millis() as u64;

    // Log with appropriate level based on performance thresholds
    if duration_ms >= thresholds.error_ms {
        error!(
            target: "server::metrics::performance",
            method = %method,
            uri = %uri,
            category = category.as_str(),
            status = %status,
            duration_ms = duration_ms,
            threshold_error_ms = thresholds.error_ms,
            client_ip = %client_ip,
            request_size = request_size,
            response_size = response_size,
            "Request exceeded error threshold"
        );
    } else if duration_ms >= thresholds.warn_ms {
        warn!(
            target: "server::metrics::performance",
            method = %method,
            uri = %uri,
            category = category.as_str(),
            status = %status,
            duration_ms = duration_ms,
            threshold_warn_ms = thresholds.warn_ms,
            client_ip = %client_ip,
            request_size = request_size,
            response_size = response_size,
            "Request exceeded warning threshold"
        );
    } else {
        info!(
            target: "server::metrics::categorized",
            method = %method,
            uri = %uri,
            category = category.as_str(),
            status = %status,
            duration_ms = duration_ms,
            client_ip = %client_ip,
            request_size = request_size,
            response_size = response_size,
            "Categorized request completed"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::*;

    #[test]
    fn test_request_categorization() {
        let test_cases = vec![
            ("/auth/login", RequestCategory::Authentication),
            ("/accounts/123", RequestCategory::UserManagement),
            ("/projects/456", RequestCategory::ProjectManagement),
            (
                "/projects/456/documents/789",
                RequestCategory::DocumentProcessing,
            ),
            ("/documents/789/inputs/", RequestCategory::FileOperations),
            ("/automation/batch", RequestCategory::Automation),
            ("/support/tickets", RequestCategory::Support),
            ("/monitors/health", RequestCategory::Monitoring),
            ("/api/swagger", RequestCategory::Api),
            ("/unknown/path", RequestCategory::Unknown),
        ];

        for (path, expected_category) in test_cases {
            let uri: Uri = path.parse().unwrap();
            let category = RequestCategory::from_uri(&uri);
            assert_eq!(category, expected_category, "Failed for path: {}", path);
        }
    }

    #[test]
    fn test_performance_thresholds() {
        let thresholds = PerformanceThresholds::for_category(&RequestCategory::Authentication);
        assert_eq!(thresholds.warn_ms, 500);
        assert_eq!(thresholds.error_ms, 2000);

        let thresholds = PerformanceThresholds::for_category(&RequestCategory::FileOperations);
        assert_eq!(thresholds.warn_ms, 5000);
        assert_eq!(thresholds.error_ms, 30000);
    }
}
