//! Document processing pipeline.
//!
//! This module provides a generic worker framework for document processing stages.
//!
//! ## Architecture
//!
//! - [`JobHandler`] - Trait for implementing stage-specific job processing
//! - [`Worker`] - Generic worker that handles subscription, concurrency, and shutdown
//! - [`WorkerHandles`] - Manages all three processing workers
//!
//! ## Stages
//!
//! - **Preprocessing**: Format validation, OCR, thumbnail generation, embeddings
//! - **Processing**: VLM-based transformations, annotations, predefined tasks
//! - **Postprocessing**: Format conversion, compression, cleanup

/// Tracing target for pipeline events.
const TRACING_TARGET: &str = "nvisy_server::pipeline";

mod job_handler;
mod postprocessing;
mod preprocessing;
mod processing;
mod state;
mod worker;

pub use job_handler::JobHandler;
pub use postprocessing::PostprocessingHandler;
pub use preprocessing::PreprocessingHandler;
pub use processing::ProcessingHandler;
pub use state::{DEFAULT_MAX_CONCURRENT_JOBS, PipelineConfig, PipelineState};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
pub use worker::Worker;

use crate::{Error, Result};

/// Type aliases for concrete worker types.
pub type PreprocessingWorker = Worker<PreprocessingHandler>;
pub type ProcessingWorker = Worker<ProcessingHandler>;
pub type PostprocessingWorker = Worker<PostprocessingHandler>;

/// Handles for background workers.
///
/// Holds join handles for all spawned workers, allowing graceful shutdown
/// and status monitoring.
pub struct WorkerHandles {
    preprocessing: JoinHandle<Result<()>>,
    processing: JoinHandle<Result<()>>,
    postprocessing: JoinHandle<Result<()>>,
    cancel_token: CancellationToken,
}

impl WorkerHandles {
    /// Spawns all document processing workers.
    ///
    /// Creates preprocessing, processing, and postprocessing workers with
    /// the given state and spawns them as background tasks. Each worker
    /// gets a unique consumer name in the format `{uuid}-{stage}`.
    ///
    /// All workers share a single semaphore for global concurrency control.
    pub fn spawn(state: &PipelineState) -> Self {
        let cancel_token = CancellationToken::new();
        let instance_id = Uuid::now_v7();
        let semaphore = state.config.create_semaphore();

        tracing::info!(
            target: TRACING_TARGET,
            instance_id = %instance_id,
            max_concurrent_jobs = state.config.max_concurrent_jobs,
            "Starting document processing workers"
        );

        let preprocessing = Worker::<PreprocessingHandler>::new(
            state.clone(),
            format!("{}-preprocessing", instance_id),
            cancel_token.clone(),
            semaphore.clone(),
        )
        .spawn();

        let processing = Worker::<ProcessingHandler>::new(
            state.clone(),
            format!("{}-processing", instance_id),
            cancel_token.clone(),
            semaphore.clone(),
        )
        .spawn();

        let postprocessing = Worker::<PostprocessingHandler>::new(
            state.clone(),
            format!("{}-postprocessing", instance_id),
            cancel_token.clone(),
            semaphore,
        )
        .spawn();

        tracing::debug!(
            target: TRACING_TARGET,
            "All workers spawned successfully"
        );

        Self {
            preprocessing,
            processing,
            postprocessing,
            cancel_token,
        }
    }

    /// Requests graceful shutdown of all workers.
    ///
    /// Workers will finish processing their current job before stopping.
    /// Use [`abort_all`](Self::abort_all) for immediate cancellation.
    pub fn shutdown(&self) {
        tracing::info!(
            target: TRACING_TARGET,
            "Initiating graceful shutdown of document processing workers"
        );
        self.cancel_token.cancel();
    }

    /// Aborts all worker tasks immediately.
    ///
    /// This cancels workers without waiting for graceful shutdown.
    /// Prefer [`shutdown`](Self::shutdown) for clean termination.
    pub fn abort_all(&self) {
        tracing::warn!(
            target: TRACING_TARGET,
            "Aborting all document processing workers immediately"
        );
        self.cancel_token.cancel();
        self.preprocessing.abort();
        self.processing.abort();
        self.postprocessing.abort();
    }

    /// Checks if all workers are still running.
    pub fn all_running(&self) -> bool {
        !self.preprocessing.is_finished()
            && !self.processing.is_finished()
            && !self.postprocessing.is_finished()
    }

    /// Checks if any worker has finished (possibly due to error).
    pub fn any_finished(&self) -> bool {
        self.preprocessing.is_finished()
            || self.processing.is_finished()
            || self.postprocessing.is_finished()
    }

    /// Waits for all workers to complete.
    ///
    /// Returns the first error encountered, if any.
    pub async fn wait_all(self) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            "Waiting for all workers to complete"
        );

        let (pre, proc, post) =
            tokio::join!(self.preprocessing, self.processing, self.postprocessing);

        pre.map_err(|e| Error::internal("pipeline", e.to_string()))??;
        proc.map_err(|e| Error::internal("pipeline", e.to_string()))??;
        post.map_err(|e| Error::internal("pipeline", e.to_string()))??;

        tracing::info!(
            target: TRACING_TARGET,
            "All document processing workers stopped"
        );

        Ok(())
    }
}
