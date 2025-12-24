//! Monitor response types.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// System monitoring status response with comprehensive health information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatus {
    /// Timestamp when this status was generated.
    pub updated_at: Timestamp,
    /// Overall system health status.
    pub is_healthy: bool,
    /// Overall system status.
    pub overall_status: SystemStatus,
    /// Application version.
    pub version: String,
    /// System uptime in milliseconds.
    pub uptime: u64,
    /// Service health details (if requested).
    pub services: Option<HashMap<String, ServiceHealth>>,
    /// System metrics (if requested).
    pub metrics: Option<SystemMetrics>,
    /// Recent errors or warnings.
    pub alerts: Option<Vec<SystemAlert>>,
}

/// System status enumeration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SystemStatus {
    /// All systems operational.
    Healthy,
    /// Some non-critical issues detected.
    Warning,
    /// Critical issues affecting functionality.
    Critical,
    /// System is degraded but operational.
    Degraded,
    /// System is down or unresponsive.
    Down,
}

/// Individual service health information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealth {
    /// Service name.
    pub name: String,
    /// Health status of the service.
    pub status: ServiceStatus,
    /// Response time in milliseconds.
    pub response_time_ms: u32,
    /// Last check timestamp.
    pub last_checked: Timestamp,
    /// Error message if unhealthy.
    pub error_message: Option<String>,
    /// Service-specific details.
    pub details: Option<serde_json::Value>,
}

/// Service status enumeration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Service is healthy and responsive.
    Healthy,
    /// Service has warnings but is functional.
    Warning,
    /// Service is unhealthy or unresponsive.
    Unhealthy,
    /// Service status is unknown.
    Unknown,
}

/// System metrics information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetrics {
    /// CPU usage percentage (0.0 to 1.0).
    pub cpu_usage: f32,
    /// Memory usage in bytes.
    pub memory_usage: u64,
    /// Total memory available in bytes.
    pub memory_total: u64,
    /// Disk usage in bytes.
    pub disk_usage: u64,
    /// Total disk space in bytes.
    pub disk_total: u64,
    /// Network statistics.
    pub network: NetworkMetrics,
    /// Application-specific metrics.
    pub application: ApplicationMetrics,
}

/// Network metrics information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMetrics {
    /// Bytes received.
    pub bytes_received: u64,
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Active connections.
    pub active_connections: u32,
}

/// Application-specific metrics.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationMetrics {
    /// Total HTTP requests processed.
    pub total_requests: u64,
    /// Requests per second (average).
    pub requests_per_second: f32,
    /// Average response time in milliseconds.
    pub avg_response_time: f32,
    /// Error rate (0.0 to 1.0).
    pub error_rate: f32,
    /// Active user sessions.
    pub active_sessions: u32,
    /// Database connection pool size.
    pub db_connections: u32,
    /// Queue size for background tasks.
    pub queue_size: u32,
}

/// System alert information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemAlert {
    /// Alert severity level.
    pub level: AlertLevel,
    /// Alert message.
    pub message: String,
    /// Component that generated the alert.
    pub component: String,
    /// Alert timestamp.
    pub timestamp: Timestamp,
    /// Additional alert context.
    pub context: Option<serde_json::Value>,
}

/// Alert severity levels.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    /// Informational message.
    Info,
    /// Warning condition.
    Warning,
    /// Error condition.
    Error,
    /// Critical condition requiring immediate attention.
    Critical,
}

/// Response for service health check operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealthResponse {
    /// Service health information.
    pub service: ServiceHealth,
    /// Deep health check results (if requested).
    pub deep_check: Option<DeepHealthCheck>,
    /// Check duration in milliseconds.
    pub check_duration_ms: u32,
}

/// Deep health check results.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeepHealthCheck {
    /// Database connectivity check.
    pub database: Option<DatabaseHealth>,
    /// Cache connectivity check.
    pub cache: Option<CacheHealth>,
    /// External API connectivity check.
    pub external_apis: Option<Vec<ExternalApiHealth>>,
    /// File system health check.
    pub filesystem: Option<FilesystemHealth>,
}

/// Database health information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealth {
    /// Connection status.
    pub is_connected: bool,
    /// Query response time in milliseconds.
    pub query_time_ms: u32,
    /// Active connections.
    pub active_connections: u32,
    /// Maximum connections.
    pub max_connections: u32,
}

/// Cache health information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CacheHealth {
    /// Connection status.
    pub is_connected: bool,
    /// Hit rate (0.0 to 1.0).
    pub hit_rate: f32,
    /// Memory usage in bytes.
    pub memory_usage: u64,
    /// Number of keys.
    pub key_count: u64,
}

/// External API health information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalApiHealth {
    /// API name or identifier.
    pub name: String,
    /// API endpoint URL.
    pub endpoint: String,
    /// Response status.
    pub is_healthy: bool,
    /// Response time in milliseconds.
    pub response_time_ms: u32,
    /// HTTP status code received.
    pub status_code: Option<u16>,
}

/// Filesystem health information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemHealth {
    /// Filesystem is accessible.
    pub is_accessible: bool,
    /// Free space in bytes.
    pub free_space: u64,
    /// Total space in bytes.
    pub total_space: u64,
    /// Read/write test successful.
    pub read_write_test: bool,
}

/// Response for metrics collection operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricsResponse {
    /// Collected metrics data.
    pub metrics: HashMap<String, MetricValue>,
    /// Time range for the metrics.
    pub time_range: String,
    /// Aggregation method used.
    pub aggregation: String,
    /// Collection timestamp.
    pub collected_at: Timestamp,
    /// Collection duration in milliseconds.
    pub collection_duration_ms: u32,
}

/// Metric value with metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricValue {
    /// Current value.
    pub current: f64,
    /// Previous value for comparison.
    pub previous: Option<f64>,
    /// Unit of measurement.
    pub unit: String,
    /// Trend direction.
    pub trend: TrendDirection,
    /// Historical data points.
    pub history: Option<Vec<HistoricalPoint>>,
}

/// Trend direction enumeration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    /// Metric is increasing.
    Up,
    /// Metric is decreasing.
    Down,
    /// Metric is stable.
    Stable,
    /// Trend is unknown.
    Unknown,
}

/// Historical data point.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalPoint {
    /// Timestamp of the data point.
    pub timestamp: Timestamp,
    /// Value at this point in time.
    pub value: f64,
}

impl Default for MonitorStatus {
    fn default() -> Self {
        Self {
            updated_at: Timestamp::now(),
            is_healthy: true,
            overall_status: SystemStatus::Healthy,
            version: "unknown".to_string(),
            uptime: 0,
            services: None,
            metrics: None,
            alerts: None,
        }
    }
}
