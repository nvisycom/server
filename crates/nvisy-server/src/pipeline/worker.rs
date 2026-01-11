//! Generic document processing worker.

use std::marker::PhantomData;
use std::sync::Arc;

use nvisy_nats::stream::{DocumentJob, DocumentJobSubscriber, Stage, TypedMessage};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{JobHandler, PipelineState};
use crate::Result;

/// Tracing target for worker infrastructure.
const TRACING_TARGET: &str = "nvisy_server::pipeline";

/// Generic document processing worker.
///
/// Handles all the boilerplate for subscribing to a NATS stream,
/// processing jobs concurrently with semaphore-based limiting,
/// and graceful shutdown via cancellation token.
///
/// The actual job processing logic is delegated to the `H: JobHandler` implementation.
pub struct Worker<H: JobHandler> {
    state: PipelineState,
    consumer_name: String,
    cancel_token: CancellationToken,
    semaphore: Arc<Semaphore>,
    _marker: PhantomData<H>,
}

impl<H: JobHandler> Worker<H> {
    /// Creates a new worker with the given handler type.
    pub fn new(
        state: PipelineState,
        consumer_name: impl Into<String>,
        cancel_token: CancellationToken,
        semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            state,
            consumer_name: consumer_name.into(),
            cancel_token,
            semaphore,
            _marker: PhantomData,
        }
    }

    /// Spawns the worker as a background task.
    pub fn spawn(self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move { self.run().await })
    }

    /// Runs the worker loop, processing jobs as they arrive.
    async fn run(self) -> Result<()> {
        tracing::info!(
            target: TRACING_TARGET,
            worker = H::WORKER_NAME,
            consumer = %self.consumer_name,
            "Starting worker"
        );

        let subscriber: DocumentJobSubscriber<H::Stage> = self
            .state
            .nats
            .document_job_subscriber(&self.consumer_name)
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            consumer = %self.consumer_name,
            stage = <H::Stage as Stage>::NAME,
            "Subscribed to jobs"
        );

        let mut stream = subscriber.subscribe().await?;

        loop {
            tokio::select! {
                biased;

                () = self.cancel_token.cancelled() => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        worker = H::WORKER_NAME,
                        "Shutdown requested, stopping worker"
                    );
                    break;
                }

                result = stream.next() => {
                    if !self.handle_stream_result(result).await {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handles a stream result, returning false if the worker should stop.
    async fn handle_stream_result(
        &self,
        result: nvisy_nats::Result<Option<TypedMessage<DocumentJob<H::Stage>>>>,
    ) -> bool {
        let msg = match result {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                tracing::trace!(target: TRACING_TARGET, "No messages available");
                return true;
            }
            Err(err) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    "Failed to receive message"
                );
                return true;
            }
        };

        // Acquire semaphore permit for concurrency control
        let permit = match self.semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    "Semaphore closed, stopping worker"
                );
                return false;
            }
        };

        let state = self.state.clone();
        let job = msg.payload().clone();
        let job_id = job.id;
        let file_id = job.file_id;

        // Ack immediately to prevent redelivery while processing
        let mut msg = msg;
        if let Err(err) = msg.ack().await {
            tracing::error!(
                target: TRACING_TARGET,
                job_id = %job_id,
                error = %err,
                "Failed to ack message"
            );
        }

        tokio::spawn(async move {
            // Hold permit until job completes
            let _permit = permit;

            tracing::info!(
                target: TRACING_TARGET,
                job_id = %job_id,
                file_id = %file_id,
                stage = <H::Stage as Stage>::NAME,
                "Processing job"
            );

            // Allow handler to log extra context
            H::log_job_start(&job);

            match H::handle_job(&state, &job).await {
                Ok(()) => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        job_id = %job_id,
                        file_id = %file_id,
                        "Job completed"
                    );
                }
                Err(err) => {
                    tracing::error!(
                        target: TRACING_TARGET,
                        job_id = %job_id,
                        file_id = %file_id,
                        error = %err,
                        "Job failed"
                    );
                    // TODO: Implement retry logic or dead letter queue
                }
            }
        });

        true
    }
}
