//! Worker configuration.

#[cfg(feature = "config")]
use clap::Args;
use nvisy_nats::NatsConfig;
use nvisy_postgres::PgConfig;
use serde::{Deserialize, Serialize};

/// Default maximum concurrent jobs per worker.
pub const DEFAULT_MAX_CONCURRENT_JOBS: usize = 10;

/// Complete worker configuration.
///
/// Combines connection configuration for external services with worker behavior settings.
/// This is the main configuration type passed to [`WorkerState::from_config`].
///
/// [`WorkerState::from_config`]: super::WorkerState::from_config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct WorkerConfig {
    /// Postgres database configuration.
    #[cfg_attr(feature = "config", command(flatten))]
    pub postgres: PgConfig,

    /// NATS configuration.
    #[cfg_attr(feature = "config", command(flatten))]
    pub nats: NatsConfig,

    /// Maximum concurrent jobs a worker can process simultaneously.
    #[cfg_attr(
        feature = "config",
        arg(
            long = "worker-max-concurrent-jobs",
            env = "WORKER_MAX_CONCURRENT_JOBS",
            default_value_t = DEFAULT_MAX_CONCURRENT_JOBS
        )
    )]
    #[serde(default = "default_max_concurrent_jobs")]
    pub max_concurrent_jobs: usize,
}

fn default_max_concurrent_jobs() -> usize {
    DEFAULT_MAX_CONCURRENT_JOBS
}

impl WorkerConfig {
    /// Creates a new worker configuration.
    pub fn new(postgres: PgConfig, nats: NatsConfig) -> Self {
        Self {
            postgres,
            nats,
            max_concurrent_jobs: DEFAULT_MAX_CONCURRENT_JOBS,
        }
    }

    /// Creates a new worker config with the specified concurrency limit.
    pub fn with_max_concurrent_jobs(mut self, max_concurrent_jobs: usize) -> Self {
        self.max_concurrent_jobs = max_concurrent_jobs;
        self
    }
}
