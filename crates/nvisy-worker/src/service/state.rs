//! Worker application state.

use std::sync::Arc;

use nvisy_inference::InferenceService;
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use tokio::sync::Semaphore;

use super::WorkerConfig;
use crate::service::config::DEFAULT_MAX_CONCURRENT_JOBS;
use crate::{Result, WorkerError};

/// Application state for workers.
///
/// Contains the services needed by document processing workers.
/// Can be created either directly with [`WorkerState::new`] or
/// from configuration with [`WorkerState::from_config`].
#[derive(Clone)]
pub struct WorkerState {
    /// PostgreSQL database client.
    pub postgres: PgClient,
    /// NATS messaging client.
    pub nats: NatsClient,
    /// AI inference service for embeddings, OCR, and VLM.
    pub inference: InferenceService,
    /// Maximum concurrent jobs a worker can process simultaneously.
    pub max_concurrent_jobs: usize,
}

impl WorkerState {
    /// Creates a new worker state from existing service instances.
    ///
    /// Use this when you already have connected clients (e.g., sharing
    /// connections with an HTTP server).
    pub fn new(postgres: PgClient, nats: NatsClient, inference: InferenceService) -> Self {
        Self {
            postgres,
            nats,
            inference,
            max_concurrent_jobs: DEFAULT_MAX_CONCURRENT_JOBS,
        }
    }

    /// Sets the maximum concurrent jobs for this worker state.
    pub fn with_max_concurrent_jobs(mut self, max_concurrent_jobs: usize) -> Self {
        self.max_concurrent_jobs = max_concurrent_jobs;
        self
    }

    /// Creates a semaphore for limiting concurrent job processing.
    pub(crate) fn create_semaphore(&self) -> Arc<Semaphore> {
        Arc::new(Semaphore::new(self.max_concurrent_jobs))
    }

    /// Creates a new worker state from configuration.
    ///
    /// Connects to PostgreSQL and NATS using the provided configuration.
    /// The inference service must be provided separately as it requires
    /// provider-specific configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if connecting to PostgreSQL or NATS fails.
    pub async fn from_config(config: &WorkerConfig, inference: InferenceService) -> Result<Self> {
        let postgres = PgClient::new(config.postgres.clone()).map_err(|e| {
            WorkerError::processing_with_source("Failed to create database client", e)
        })?;

        let nats = NatsClient::connect(config.nats.clone())
            .await
            .map_err(|e| WorkerError::processing_with_source("Failed to connect to NATS", e))?;

        Ok(Self {
            postgres,
            nats,
            inference,
            max_concurrent_jobs: config.max_concurrent_jobs,
        })
    }
}
