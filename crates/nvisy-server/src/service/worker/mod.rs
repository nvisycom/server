//! Background workers for document processing pipeline.
//!
//! Workers subscribe to NATS JetStream subjects and process document jobs
//! concurrently. Each worker handles a specific stage of the pipeline:
//!
//! - [`PreprocessingWorker`] - Runs on file upload (OCR, thumbnails, embeddings)
//! - [`ProcessingWorker`] - Runs on edit requests (VLM-based transformations)
//! - [`PostprocessingWorker`] - Runs on download (format conversion, compression)

mod error;
mod postprocessing;
mod preprocessing;
mod processing;

use std::sync::Arc;

#[cfg(any(test, feature = "config"))]
use clap::Args;
pub use error::{Result, WorkerError};
use postprocessing::PostprocessingWorker;
use preprocessing::PreprocessingWorker;
use processing::ProcessingWorker;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::service::ServiceState;

/// Default maximum concurrent jobs per worker.
const DEFAULT_MAX_CONCURRENT_JOBS: usize = 10;

/// Configuration for worker behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "config"), derive(Args))]
pub struct WorkerConfig {
    /// Maximum concurrent jobs a worker can process simultaneously.
    #[cfg_attr(
        any(test, feature = "config"),
        arg(
            long = "worker-max-concurrent-jobs",
            env = "WORKER_MAX_CONCURRENT_JOBS",
            default_value_t = DEFAULT_MAX_CONCURRENT_JOBS
        )
    )]
    #[serde(default = "WorkerConfig::default_max_concurrent_jobs")]
    pub max_concurrent_jobs: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: DEFAULT_MAX_CONCURRENT_JOBS,
        }
    }
}

impl WorkerConfig {
    /// Creates a new worker config with the specified concurrency limit.
    pub fn with_max_concurrent_jobs(max_concurrent_jobs: usize) -> Self {
        Self {
            max_concurrent_jobs,
        }
    }

    fn default_max_concurrent_jobs() -> usize {
        DEFAULT_MAX_CONCURRENT_JOBS
    }

    /// Creates a semaphore for limiting concurrent job processing.
    pub(crate) fn create_semaphore(&self) -> Arc<Semaphore> {
        Arc::new(Semaphore::new(self.max_concurrent_jobs))
    }
}

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
    /// the given configuration and spawns them as background tasks.
    pub fn spawn(state: &ServiceState, config: &WorkerConfig) -> Self {
        let cancel_token = CancellationToken::new();

        let preprocessing = PreprocessingWorker::new(
            state.clone(),
            "preprocessing-worker",
            cancel_token.clone(),
            config,
        )
        .spawn();

        let processing = ProcessingWorker::new(
            state.clone(),
            "processing-worker",
            cancel_token.clone(),
            config,
        )
        .spawn();

        let postprocessing = PostprocessingWorker::new(
            state.clone(),
            "postprocessing-worker",
            cancel_token.clone(),
            config,
        )
        .spawn();

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
        self.cancel_token.cancel();
    }

    /// Aborts all worker tasks immediately.
    ///
    /// This cancels workers without waiting for graceful shutdown.
    /// Prefer [`shutdown`](Self::shutdown) for clean termination.
    pub fn abort_all(&self) {
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
        let (pre, proc, post) =
            tokio::join!(self.preprocessing, self.processing, self.postprocessing,);

        // Return first error encountered
        pre.map_err(|e| WorkerError::processing(e.to_string()))??;
        proc.map_err(|e| WorkerError::processing(e.to_string()))??;
        post.map_err(|e| WorkerError::processing(e.to_string()))??;

        Ok(())
    }
}
