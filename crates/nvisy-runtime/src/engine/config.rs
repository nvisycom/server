//! Engine configuration.

use std::time::Duration;

use derive_builder::Builder;

/// Configuration for the workflow execution engine.
#[derive(Debug, Clone, Builder)]
#[builder(setter(into), build_fn(validate = "Self::validate"))]
pub struct EngineConfig {
    /// Maximum number of concurrent workflow executions.
    #[builder(default = "10")]
    pub max_concurrent_runs: usize,

    /// Default timeout for workflow execution.
    #[builder(default = "Duration::from_secs(3600)")]
    pub default_timeout: Duration,

    /// Maximum number of retries for failed nodes.
    #[builder(default = "3")]
    pub max_retries: u32,

    /// Delay between retries.
    #[builder(default = "Duration::from_secs(1)")]
    pub retry_delay: Duration,
}

impl EngineConfigBuilder {
    fn validate(&self) -> Result<(), String> {
        if let Some(max) = self.max_concurrent_runs {
            if max == 0 {
                return Err("max_concurrent_runs must be at least 1".into());
            }
        }
        Ok(())
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_runs: 10,
            default_timeout: Duration::from_secs(3600),
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
        }
    }
}
