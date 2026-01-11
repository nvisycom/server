//! Pipeline state and configuration.

use std::sync::Arc;

use clap::Args;
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::service::ServiceState;

/// Default maximum concurrent jobs.
pub const DEFAULT_MAX_CONCURRENT_JOBS: usize = 10;

/// Configuration for the document processing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, Args)]
pub struct PipelineConfig {
    /// Maximum concurrent jobs workers can process simultaneously.
    #[arg(long, env = "PIPELINE_MAX_CONCURRENT_JOBS", default_value_t = DEFAULT_MAX_CONCURRENT_JOBS)]
    pub max_concurrent_jobs: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: DEFAULT_MAX_CONCURRENT_JOBS,
        }
    }
}

impl PipelineConfig {
    /// Creates a new pipeline configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum concurrent jobs.
    pub fn with_max_concurrent_jobs(mut self, max_concurrent_jobs: usize) -> Self {
        self.max_concurrent_jobs = max_concurrent_jobs;
        self
    }

    /// Creates a semaphore for limiting concurrent job processing.
    pub fn create_semaphore(&self) -> Arc<Semaphore> {
        Arc::new(Semaphore::new(self.max_concurrent_jobs))
    }
}

/// Application state for pipeline workers.
///
/// Contains the services needed by document processing workers.
#[derive(Clone)]
pub struct PipelineState {
    /// PostgreSQL database client.
    pub postgres: PgClient,
    /// NATS messaging client.
    pub nats: NatsClient,
    /// Pipeline configuration.
    pub config: PipelineConfig,
}

impl PipelineState {
    /// Creates a new pipeline state from service state and configuration.
    pub fn new(state: &ServiceState, config: PipelineConfig) -> Self {
        Self {
            postgres: state.postgres.clone(),
            nats: state.nats.clone(),
            config,
        }
    }
}
