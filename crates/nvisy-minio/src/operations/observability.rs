//! Comprehensive observability module for MinIO operations.
//!
//! This module provides advanced monitoring, metrics collection, health checks,
//! and diagnostic capabilities for MinIO operations beyond basic tracing.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::operations::{DownloadResult, UploadResult};
// Removed unused imports: DownloadContext, UploadContext
use crate::{Error, TRACING_TARGET_OPERATIONS};

/// Comprehensive metrics collector for MinIO operations.
#[derive(Debug, Clone)]
pub struct OperationMetrics {
    /// Total number of operations attempted.
    pub operations_total: Arc<AtomicU64>,
    /// Total number of successful operations.
    pub operations_success: Arc<AtomicU64>,
    /// Total number of failed operations.
    pub operations_failed: Arc<AtomicU64>,
    /// Total bytes uploaded.
    pub bytes_uploaded: Arc<AtomicU64>,
    /// Total bytes downloaded.
    pub bytes_downloaded: Arc<AtomicU64>,
    /// Total upload duration in milliseconds.
    pub upload_duration_ms: Arc<AtomicU64>,
    /// Total download duration in milliseconds.
    pub download_duration_ms: Arc<AtomicU64>,
    /// Number of retries performed.
    pub retries_total: Arc<AtomicU64>,
    /// Number of timeouts encountered.
    pub timeouts_total: Arc<AtomicU64>,
    /// Error counters by type.
    pub error_counters: Arc<HashMap<String, AtomicU64>>,
}

impl Default for OperationMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationMetrics {
    /// Creates a new metrics collector.
    pub fn new() -> Self {
        Self {
            operations_total: Arc::new(AtomicU64::new(0)),
            operations_success: Arc::new(AtomicU64::new(0)),
            operations_failed: Arc::new(AtomicU64::new(0)),
            bytes_uploaded: Arc::new(AtomicU64::new(0)),
            bytes_downloaded: Arc::new(AtomicU64::new(0)),
            upload_duration_ms: Arc::new(AtomicU64::new(0)),
            download_duration_ms: Arc::new(AtomicU64::new(0)),
            retries_total: Arc::new(AtomicU64::new(0)),
            timeouts_total: Arc::new(AtomicU64::new(0)),
            error_counters: Arc::new(HashMap::new()),
        }
    }

    /// Records a successful upload operation.
    pub fn record_upload_success(&self, result: &UploadResult) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.operations_success.fetch_add(1, Ordering::Relaxed);
        self.bytes_uploaded
            .fetch_add(result.size, Ordering::Relaxed);
        self.upload_duration_ms
            .fetch_add(result.duration.as_millis() as u64, Ordering::Relaxed);

        debug!(
            target: TRACING_TARGET_OPERATIONS,
            operation = "upload_success",
            key = %result.key,
            size = result.size,
            duration_ms = result.duration.as_millis(),
            throughput_mbps = (result.size as f64 / result.duration.as_secs_f64()) / (1024.0 * 1024.0),
            "Upload operation completed successfully"
        );
    }

    /// Records a successful download operation.
    pub fn record_download_success(&self, result: &DownloadResult) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.operations_success.fetch_add(1, Ordering::Relaxed);
        self.bytes_downloaded
            .fetch_add(result.size, Ordering::Relaxed);
        self.download_duration_ms
            .fetch_add(result.duration.as_millis() as u64, Ordering::Relaxed);

        debug!(
            target: TRACING_TARGET_OPERATIONS,
            operation = "download_success",
            key = %result.key,
            size = result.size,
            duration_ms = result.duration.as_millis(),
            throughput_mbps = (result.size as f64 / result.duration.as_secs_f64()) / (1024.0 * 1024.0),
            "Download operation completed successfully"
        );
    }

    /// Records a failed operation.
    pub fn record_operation_failure(&self, error: &Error, operation_type: &str) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.operations_failed.fetch_add(1, Ordering::Relaxed);

        let error_type = match error {
            Error::Config(_) => "config",
            Error::InvalidRequest(_) => "invalid_request",
            Error::NotFound(_) => "not_found",
            Error::TransientNetwork(_) => "transient_network",
            Error::RateLimited { .. } => "rate_limited",
            Error::QuotaExceeded { .. } => "quota_exceeded",
            Error::ChecksumMismatch { .. } => "checksum_mismatch",
            Error::Timeout { .. } => "timeout",
            Error::ServerError { .. } => "server_error",
            Error::Serialization(_) => "serialization",
            Error::Io(_) => "io",
            Error::Client(_) => "client",
        };

        // This would need to be updated with thread-safe HashMap
        // For now, we'll just log the error type
        warn!(
            target: TRACING_TARGET_OPERATIONS,
            operation = operation_type,
            error_type = error_type,
            error = %error,
            "Operation failed"
        );
    }

    /// Records a retry attempt.
    pub fn record_retry(&self, attempt: u32, delay: Duration) {
        self.retries_total.fetch_add(1, Ordering::Relaxed);

        debug!(
            target: TRACING_TARGET_OPERATIONS,
            attempt = attempt,
            delay_ms = delay.as_millis(),
            "Operation retry attempted"
        );
    }

    /// Records a timeout occurrence.
    pub fn record_timeout(&self, operation_type: &str, timeout_duration: Duration) {
        self.timeouts_total.fetch_add(1, Ordering::Relaxed);

        warn!(
            target: TRACING_TARGET_OPERATIONS,
            operation = operation_type,
            timeout_ms = timeout_duration.as_millis(),
            "Operation timeout"
        );
    }

    /// Gets current metrics snapshot.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            operations_total: self.operations_total.load(Ordering::Relaxed),
            operations_success: self.operations_success.load(Ordering::Relaxed),
            operations_failed: self.operations_failed.load(Ordering::Relaxed),
            bytes_uploaded: self.bytes_uploaded.load(Ordering::Relaxed),
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
            upload_duration_ms: self.upload_duration_ms.load(Ordering::Relaxed),
            download_duration_ms: self.download_duration_ms.load(Ordering::Relaxed),
            retries_total: self.retries_total.load(Ordering::Relaxed),
            timeouts_total: self.timeouts_total.load(Ordering::Relaxed),
            timestamp: SystemTime::now(),
        }
    }

    /// Resets all metrics counters.
    pub fn reset(&self) {
        self.operations_total.store(0, Ordering::Relaxed);
        self.operations_success.store(0, Ordering::Relaxed);
        self.operations_failed.store(0, Ordering::Relaxed);
        self.bytes_uploaded.store(0, Ordering::Relaxed);
        self.bytes_downloaded.store(0, Ordering::Relaxed);
        self.upload_duration_ms.store(0, Ordering::Relaxed);
        self.download_duration_ms.store(0, Ordering::Relaxed);
        self.retries_total.store(0, Ordering::Relaxed);
        self.timeouts_total.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of metrics at a specific point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Total operations attempted.
    pub operations_total: u64,
    /// Successful operations.
    pub operations_success: u64,
    /// Failed operations.
    pub operations_failed: u64,
    /// Total bytes uploaded.
    pub bytes_uploaded: u64,
    /// Total bytes downloaded.
    pub bytes_downloaded: u64,
    /// Total upload duration in milliseconds.
    pub upload_duration_ms: u64,
    /// Total download duration in milliseconds.
    pub download_duration_ms: u64,
    /// Total retries performed.
    pub retries_total: u64,
    /// Total timeouts encountered.
    pub timeouts_total: u64,
    /// Timestamp when snapshot was taken.
    pub timestamp: SystemTime,
}

impl MetricsSnapshot {
    /// Calculates success rate as a percentage.
    pub fn success_rate(&self) -> f64 {
        if self.operations_total > 0 {
            (self.operations_success as f64 / self.operations_total as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Calculates failure rate as a percentage.
    pub fn failure_rate(&self) -> f64 {
        if self.operations_total > 0 {
            (self.operations_failed as f64 / self.operations_total as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Calculates average upload throughput in MB/s.
    pub fn avg_upload_throughput_mbps(&self) -> f64 {
        if self.upload_duration_ms > 0 {
            let duration_seconds = self.upload_duration_ms as f64 / 1000.0;
            let bytes_per_second = self.bytes_uploaded as f64 / duration_seconds;
            bytes_per_second / (1024.0 * 1024.0)
        } else {
            0.0
        }
    }

    /// Calculates average download throughput in MB/s.
    pub fn avg_download_throughput_mbps(&self) -> f64 {
        if self.download_duration_ms > 0 {
            let duration_seconds = self.download_duration_ms as f64 / 1000.0;
            let bytes_per_second = self.bytes_downloaded as f64 / duration_seconds;
            bytes_per_second / (1024.0 * 1024.0)
        } else {
            0.0
        }
    }

    /// Gets the age of this snapshot.
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
    }
}

/// Health status for MinIO operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// All systems operational.
    Healthy,
    /// Minor issues but functional.
    Degraded,
    /// Major issues affecting functionality.
    Unhealthy,
    /// Unable to determine status.
    Unknown,
}

/// Health check result with details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Overall health status.
    pub status: HealthStatus,
    /// Timestamp of the health check.
    pub timestamp: SystemTime,
    /// Detailed health information.
    pub details: HashMap<String, String>,
    /// Duration of the health check.
    pub check_duration: Duration,
}

impl HealthCheck {
    /// Creates a new health check result.
    pub fn new(status: HealthStatus, check_duration: Duration) -> Self {
        Self {
            status,
            timestamp: SystemTime::now(),
            details: HashMap::new(),
            check_duration,
        }
    }

    /// Adds a detail to the health check.
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    /// Checks if the health status is healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, HealthStatus::Healthy)
    }

    /// Checks if the health status indicates problems.
    pub fn has_issues(&self) -> bool {
        matches!(
            self.status,
            HealthStatus::Degraded | HealthStatus::Unhealthy
        )
    }
}

/// Performance monitor that tracks operation patterns and performance.
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// Operation metrics.
    metrics: OperationMetrics,
    /// Recent operation times for trend analysis.
    recent_operations: Vec<(Instant, Duration, bool)>, // (time, duration, success)
    /// Performance thresholds.
    thresholds: PerformanceThresholds,
}

/// Performance thresholds for alerting.
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    /// Maximum acceptable failure rate (0.0 to 1.0).
    pub max_failure_rate: f64,
    /// Maximum acceptable average response time.
    pub max_avg_response_time: Duration,
    /// Minimum acceptable throughput in MB/s.
    pub min_throughput_mbps: f64,
    /// Maximum acceptable timeout rate.
    pub max_timeout_rate: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_failure_rate: 0.05, // 5%
            max_avg_response_time: Duration::from_secs(30),
            min_throughput_mbps: 1.0, // 1 MB/s minimum
            max_timeout_rate: 0.01,   // 1%
        }
    }
}

impl PerformanceMonitor {
    /// Creates a new performance monitor.
    pub fn new() -> Self {
        Self::with_thresholds(PerformanceThresholds::default())
    }

    /// Creates a new performance monitor with custom thresholds.
    pub fn with_thresholds(thresholds: PerformanceThresholds) -> Self {
        Self {
            metrics: OperationMetrics::new(),
            recent_operations: Vec::new(),
            thresholds,
        }
    }

    /// Records an operation completion.
    pub fn record_operation(&mut self, duration: Duration, success: bool) {
        let now = Instant::now();
        self.recent_operations.push((now, duration, success));

        // Keep only recent operations (last 1000 or last 5 minutes)
        let cutoff = now - Duration::from_secs(300); // 5 minutes
        let current_len = self.recent_operations.len();
        self.recent_operations
            .retain(|(time, _, _)| *time > cutoff || current_len <= 1000);

        if self.recent_operations.len() > 1000 {
            let excess = self.recent_operations.len() - 1000;
            self.recent_operations.drain(..excess);
        }

        // Log performance warnings
        if success && duration > self.thresholds.max_avg_response_time {
            warn!(
                target: TRACING_TARGET_OPERATIONS,
                duration_ms = duration.as_millis(),
                threshold_ms = self.thresholds.max_avg_response_time.as_millis(),
                "Operation exceeded response time threshold"
            );
        }
    }

    /// Gets current performance analysis.
    pub fn analyze_performance(&self) -> PerformanceAnalysis {
        let total_ops = self.recent_operations.len();
        if total_ops == 0 {
            return PerformanceAnalysis::default();
        }

        let successful_ops = self
            .recent_operations
            .iter()
            .filter(|(_, _, success)| *success)
            .count();
        let failed_ops = total_ops - successful_ops;

        let total_duration: Duration = self
            .recent_operations
            .iter()
            .map(|(_, duration, _)| *duration)
            .sum();

        let avg_response_time = total_duration / total_ops as u32;
        let failure_rate = failed_ops as f64 / total_ops as f64;

        let mut issues = Vec::new();

        if failure_rate > self.thresholds.max_failure_rate {
            issues.push(format!(
                "High failure rate: {:.1}% (threshold: {:.1}%)",
                failure_rate * 100.0,
                self.thresholds.max_failure_rate * 100.0
            ));
        }

        if avg_response_time > self.thresholds.max_avg_response_time {
            issues.push(format!(
                "High response time: {:?} (threshold: {:?})",
                avg_response_time, self.thresholds.max_avg_response_time
            ));
        }

        let health_status = if issues.is_empty() {
            HealthStatus::Healthy
        } else if failure_rate > self.thresholds.max_failure_rate * 2.0 {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };

        PerformanceAnalysis {
            total_operations: total_ops,
            successful_operations: successful_ops,
            failed_operations: failed_ops,
            failure_rate,
            avg_response_time,
            health_status,
            issues,
        }
    }

    /// Gets reference to metrics.
    pub fn metrics(&self) -> &OperationMetrics {
        &self.metrics
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Analysis of recent performance data.
#[derive(Debug, Clone)]
pub struct PerformanceAnalysis {
    /// Total number of operations analyzed.
    pub total_operations: usize,
    /// Number of successful operations.
    pub successful_operations: usize,
    /// Number of failed operations.
    pub failed_operations: usize,
    /// Failure rate (0.0 to 1.0).
    pub failure_rate: f64,
    /// Average response time.
    pub avg_response_time: Duration,
    /// Current health status based on analysis.
    pub health_status: HealthStatus,
    /// List of identified performance issues.
    pub issues: Vec<String>,
}

impl Default for PerformanceAnalysis {
    fn default() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            failure_rate: 0.0,
            avg_response_time: Duration::from_secs(0),
            health_status: HealthStatus::Unknown,
            issues: Vec::new(),
        }
    }
}

/// Diagnostic information collector.
#[derive(Debug, Clone)]
pub struct DiagnosticInfo {
    /// Environment information.
    pub environment: HashMap<String, String>,
    /// Configuration summary.
    pub configuration: HashMap<String, String>,
    /// Recent errors and warnings.
    pub recent_errors: Vec<String>,
    /// System resource usage.
    pub resource_usage: ResourceUsage,
}

/// Resource usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ResourceUsage {
    /// Memory usage in bytes (if available).
    pub memory_bytes: Option<u64>,
    /// CPU usage percentage (if available).
    pub cpu_percentage: Option<f64>,
    /// Number of open file descriptors (if available).
    pub open_file_descriptors: Option<u64>,
    /// Network connections count (if available).
    pub network_connections: Option<u64>,
}

impl DiagnosticInfo {
    /// Creates a new diagnostic info collector.
    pub fn new() -> Self {
        let mut environment = HashMap::new();
        environment.insert(
            "rust_version".to_string(),
            env!("CARGO_PKG_RUST_VERSION").to_string(),
        );
        environment.insert(
            "crate_version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );

        if let Ok(hostname) = std::env::var("HOSTNAME") {
            environment.insert("hostname".to_string(), hostname);
        }

        Self {
            environment,
            configuration: HashMap::new(),
            recent_errors: Vec::new(),
            resource_usage: ResourceUsage::default(),
        }
    }

    /// Adds configuration information.
    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.configuration.insert(key.into(), value.into());
        self
    }

    /// Adds an error to recent errors list.
    pub fn add_error(&mut self, error: String) {
        self.recent_errors.push(format!(
            "[{}] {}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            error
        ));

        // Keep only recent errors (last 100)
        if self.recent_errors.len() > 100 {
            self.recent_errors.drain(..self.recent_errors.len() - 100);
        }
    }

    /// Updates resource usage information.
    pub fn update_resource_usage(&mut self, usage: ResourceUsage) {
        self.resource_usage = usage;
    }
}

impl Default for DiagnosticInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = OperationMetrics::new();

        let upload_result = UploadResult {
            key: "test-key".to_string(),
            size: 1024,
            etag: "test-etag".to_string(),
            duration: Duration::from_millis(100),
        };

        metrics.record_upload_success(&upload_result);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.operations_total, 1);
        assert_eq!(snapshot.operations_success, 1);
        assert_eq!(snapshot.bytes_uploaded, 1024);
    }

    #[test]
    fn test_metrics_snapshot_calculations() {
        let snapshot = MetricsSnapshot {
            operations_total: 100,
            operations_success: 95,
            operations_failed: 5,
            bytes_uploaded: 1024 * 1024,       // 1MB
            bytes_downloaded: 2 * 1024 * 1024, // 2MB
            upload_duration_ms: 1000,          // 1 second
            download_duration_ms: 2000,        // 2 seconds
            retries_total: 10,
            timeouts_total: 2,
            timestamp: SystemTime::now(),
        };

        assert_eq!(snapshot.success_rate(), 95.0);
        assert_eq!(snapshot.failure_rate(), 5.0);
        assert_eq!(snapshot.avg_upload_throughput_mbps(), 1.0); // 1MB/1s = 1MB/s
        assert_eq!(snapshot.avg_download_throughput_mbps(), 1.0); // 2MB/2s = 1MB/s
    }

    #[test]
    fn test_health_check() {
        let health = HealthCheck::new(HealthStatus::Healthy, Duration::from_millis(50))
            .with_detail("status", "All systems operational")
            .with_detail("uptime", "24h");

        assert!(health.is_healthy());
        assert!(!health.has_issues());
        assert_eq!(
            health.details.get("status").unwrap(),
            "All systems operational"
        );
    }

    #[test]
    fn test_performance_monitor() {
        let mut monitor = PerformanceMonitor::new();

        // Record some successful operations
        monitor.record_operation(Duration::from_millis(100), true);
        monitor.record_operation(Duration::from_millis(200), true);
        monitor.record_operation(Duration::from_millis(150), true);

        // Record one failure
        monitor.record_operation(Duration::from_millis(50), false);

        let analysis = monitor.analyze_performance();
        assert_eq!(analysis.total_operations, 4);
        assert_eq!(analysis.successful_operations, 3);
        assert_eq!(analysis.failed_operations, 1);
        assert_eq!(analysis.failure_rate, 0.25);
    }
}
