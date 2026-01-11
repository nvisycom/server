//! Job handler trait for stage-specific processing logic.

use std::future::Future;

use nvisy_nats::stream::{DocumentJob, Stage};

use super::PipelineState;
use crate::Result;

/// Trait for implementing stage-specific job processing logic.
///
/// Each processing stage implements this trait to define how jobs
/// are handled. The framework takes care of subscription, concurrency,
/// shutdown, and error handling.
///
/// # Example
///
/// ```ignore
/// pub struct MyHandler;
///
/// impl JobHandler for MyHandler {
///     type Stage = MyStageData;
///     const TRACING_TARGET: &'static str = "my_worker::stage";
///     const WORKER_NAME: &'static str = "my_stage";
///
///     async fn handle_job(state: &PipelineState, job: &DocumentJob<Self::Stage>) -> Result<()> {
///         // Process the job
///         Ok(())
///     }
/// }
/// ```
pub trait JobHandler: Send + Sync + 'static {
    /// The processing stage this handler operates on.
    type Stage: Stage;

    /// Tracing target for this handler's log messages.
    const TRACING_TARGET: &'static str;

    /// Human-readable name for this worker (used in logs).
    const WORKER_NAME: &'static str;

    /// Process a single job.
    ///
    /// This is the only method that stage-specific implementations need to define.
    /// The framework handles message acknowledgment, concurrency control, and
    /// error logging.
    fn handle_job(
        state: &PipelineState,
        job: &DocumentJob<Self::Stage>,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Optional: Log additional context when a job starts.
    ///
    /// Override this to add stage-specific fields to the "Processing job" log.
    /// Default implementation logs nothing extra.
    #[inline]
    fn log_job_start(_job: &DocumentJob<Self::Stage>) {
        // Default: no extra logging
    }
}
