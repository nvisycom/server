#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
pub mod handler;
pub mod service;

pub use error::{Result, WorkerError};
pub use handler::{PostprocessingWorker, PreprocessingWorker, ProcessingWorker};
pub use service::{WorkerConfig, WorkerState};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

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
    /// the given state and spawns them as background tasks.
    pub fn spawn(state: &WorkerState) -> Self {
        let cancel_token = CancellationToken::new();

        let preprocessing =
            PreprocessingWorker::new(state.clone(), "preprocessing-worker", cancel_token.clone())
                .spawn();

        let processing =
            ProcessingWorker::new(state.clone(), "processing-worker", cancel_token.clone()).spawn();

        let postprocessing =
            PostprocessingWorker::new(state.clone(), "postprocessing-worker", cancel_token.clone())
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
