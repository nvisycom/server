//! Component implementation for OpenRouter client health monitoring

use nvisy_error::{Component, ComponentStatus, HealthStatus, OperationalState, UpdateSeverity};

use super::client::LlmClient;
use super::error::Error;
use crate::OPENROUTER_TARGET;

impl Component for LlmClient {
    async fn current_status(&self) -> ComponentStatus {
        // Check OpenRouter API health by attempting to list models
        match self.list_models().await {
            Ok(models) => {
                let model_count = models.len();

                if model_count > 0 {
                    ComponentStatus::new(HealthStatus::Online)
                        .with_operational_state(OperationalState::Running)
                        .with_update_severity(UpdateSeverity::Info)
                        .with_message("OpenRouter API is healthy")
                        .with_details(format!("Successfully retrieved {} models", model_count))
                } else {
                    // Models list is empty, which might indicate an issue
                    ComponentStatus::new(HealthStatus::MinorDegraded)
                        .with_operational_state(OperationalState::Running)
                        .with_update_severity(UpdateSeverity::Warning)
                        .with_message("OpenRouter API responding but no models available")
                        .with_details("Models list is empty")
                }
            }
            Err(error) => {
                tracing::error!(
                    target: OPENROUTER_TARGET,
                    error = %error,
                    "OpenRouter API health check failed"
                );

                let (health_status, operational_state, severity) = match &error {
                    Error::Auth { .. } => (
                        HealthStatus::Offline,
                        OperationalState::Stopped,
                        UpdateSeverity::Critical,
                    ),
                    Error::RateLimit { .. } => (
                        HealthStatus::MinorDegraded,
                        OperationalState::Running,
                        UpdateSeverity::Warning,
                    ),
                    Error::Network {
                        recoverable: true, ..
                    } => (
                        HealthStatus::MinorDegraded,
                        OperationalState::Running,
                        UpdateSeverity::Warning,
                    ),
                    Error::Network {
                        recoverable: false, ..
                    } => (
                        HealthStatus::Offline,
                        OperationalState::Stopped,
                        UpdateSeverity::Critical,
                    ),
                    Error::Timeout { .. } => (
                        HealthStatus::MinorDegraded,
                        OperationalState::Running,
                        UpdateSeverity::Warning,
                    ),
                    Error::Config { .. } => (
                        HealthStatus::Offline,
                        OperationalState::Stopped,
                        UpdateSeverity::Critical,
                    ),

                    Error::Api {
                        status_code: Some(code),
                        ..
                    } => {
                        if *code >= 500 {
                            (
                                HealthStatus::MajorDegraded,
                                OperationalState::Running,
                                UpdateSeverity::Warning,
                            )
                        } else if *code >= 400 {
                            (
                                HealthStatus::Offline,
                                OperationalState::Stopped,
                                UpdateSeverity::Critical,
                            )
                        } else {
                            (
                                HealthStatus::MinorDegraded,
                                OperationalState::Running,
                                UpdateSeverity::Warning,
                            )
                        }
                    }
                    _ => (
                        HealthStatus::MajorDegraded,
                        OperationalState::Running,
                        UpdateSeverity::Warning,
                    ),
                };

                let (current_rate_limit, max_rate_limit) = self.rate_limit_status();
                let rate_limit_info = if current_rate_limit == 0 {
                    format!(
                        " (Rate limit: {}/{} - exhausted)",
                        current_rate_limit, max_rate_limit
                    )
                } else {
                    format!(
                        " (Rate limit: {}/{} - available)",
                        current_rate_limit, max_rate_limit
                    )
                };

                ComponentStatus::new(health_status)
                    .with_operational_state(operational_state)
                    .with_update_severity(severity)
                    .with_message("OpenRouter API health check failed")
                    .with_details(format!("Error: {}{}", error, rate_limit_info))
            }
        }
    }

    async fn cached_status(&self) -> Option<ComponentStatus> {
        // For now, return None to always check current status
        // In a real implementation, this could cache the status for a few minutes
        // to avoid hitting rate limits too frequently during health checks
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::config::LlmConfig;

    #[tokio::test]
    async fn test_component_status_structure() {
        // This test ensures the component status has the correct structure
        // We can't test the actual API calls without a valid key
        let _config = LlmConfig::default();

        // Test that we can create a proper component status manually
        let status = ComponentStatus::new(HealthStatus::Online)
            .with_operational_state(OperationalState::Running)
            .with_update_severity(UpdateSeverity::Info)
            .with_message("Test message");

        assert_eq!(status.health_status, HealthStatus::Online);
        assert_eq!(status.operational_state, OperationalState::Running);
        assert_eq!(status.update_severity, UpdateSeverity::Info);
    }
}
