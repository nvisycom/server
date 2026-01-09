//! Document processing handlers.
//!
//! This module contains workers for each stage of document processing:
//!
//! - **Preprocessing**: Format validation, OCR, thumbnail generation, embeddings
//! - **Processing**: VLM-based transformations, annotations, predefined tasks
//! - **Postprocessing**: Format conversion, compression, cleanup

mod postprocessing;
mod preprocessing;
mod processing;

pub use postprocessing::PostprocessingWorker;
pub use preprocessing::PreprocessingWorker;
pub use processing::ProcessingWorker;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::service::WorkerState;
use crate::{Result, WorkerError};

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
    /// gets a unique consumer name in the format `{uuid}-{worker_type}`.
    pub fn spawn(state: &WorkerState) -> Self {
        let cancel_token = CancellationToken::new();
        let instance_id = Uuid::now_v7();

        let preprocessing = PreprocessingWorker::new(
            state.clone(),
            format!("{}-preprocessing", instance_id),
            cancel_token.clone(),
        )
        .spawn();

        let processing = ProcessingWorker::new(
            state.clone(),
            format!("{}-processing", instance_id),
            cancel_token.clone(),
        )
        .spawn();

        let postprocessing = PostprocessingWorker::new(
            state.clone(),
            format!("{}-postprocessing", instance_id),
            cancel_token.clone(),
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
