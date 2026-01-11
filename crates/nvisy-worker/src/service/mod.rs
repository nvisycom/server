//! Worker state and services.
//!
//! This module provides the core state management for document processing workers,
//! including configuration for concurrency limits and connections to external services.
//!
//! ## Services
//!
//! - [`TextSplitterService`] - Semantic text chunking for embeddings
//! - [`MetadataService`] - Document metadata extraction and validation
//! - [`ThumbnailService`] - Thumbnail generation at various sizes
//! - [`FormatConversionService`] - Document format conversion

mod config;
mod format_conversion;
mod metadata;
mod text_splitter;
mod thumbnail;

use std::sync::Arc;

pub use config::WorkerConfig;
pub use format_conversion::{
    ConversionOptions, ConversionResult, DocumentFormat, FormatConversionService, PageRange,
};
pub use metadata::{
    DocumentMetadata, IssueSeverity, MetadataService, ValidationIssue, ValidationResult,
};
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
pub use text_splitter::{ChunkStream, TextChunk, TextSplitterConfig, TextSplitterService};
pub use thumbnail::{
    Thumbnail, ThumbnailFormat, ThumbnailOptions, ThumbnailService, ThumbnailSize,
};
use tokio::sync::Semaphore;

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
    /// Maximum concurrent jobs a worker can process simultaneously.
    pub max_concurrent_jobs: usize,
}

impl WorkerState {
    /// Creates a new worker state from existing service instances.
    ///
    /// Use this when you already have connected clients (e.g., sharing
    /// connections with an HTTP server).
    pub fn new(postgres: PgClient, nats: NatsClient) -> Self {
        Self {
            postgres,
            nats,
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
    ///
    /// # Errors
    ///
    /// Returns an error if connecting to PostgreSQL or NATS fails.
    pub async fn from_config(config: &WorkerConfig) -> Result<Self> {
        let postgres = PgClient::new(config.postgres.clone()).map_err(|e| {
            WorkerError::processing_with_source("Failed to create database client", e)
        })?;

        let nats = NatsClient::connect(config.nats.clone())
            .await
            .map_err(|e| WorkerError::processing_with_source("Failed to connect to NATS", e))?;

        Ok(Self {
            postgres,
            nats,
            max_concurrent_jobs: config.max_concurrent_jobs,
        })
    }
}
