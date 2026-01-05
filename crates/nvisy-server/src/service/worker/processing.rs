//! Processing worker for document editing pipeline.
//!
//! Handles jobs triggered by edit requests:
//! - VLM-based document transformations
//! - Annotation processing
//! - Predefined tasks (redaction, translation, summarization, etc.)

use std::sync::Arc;

use nvisy_nats::stream::{DocumentJob, DocumentJobSubscriber, ProcessingData};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::WorkerConfig;
use super::error::Result;
use crate::service::ServiceState;

/// Tracing target for processing worker.
const TRACING_TARGET: &str = "nvisy_server::worker::processing";

/// Background worker for processing document jobs.
///
/// Subscribes to processing stage jobs and executes VLM-based
/// transformations on documents. Handles annotation application,
/// predefined tasks, and custom processing instructions.
pub struct ProcessingWorker {
    state: ServiceState,
    consumer_name: String,
    cancel_token: CancellationToken,
    semaphore: Arc<Semaphore>,
}

impl ProcessingWorker {
    /// Creates a new processing worker.
    ///
    /// # Arguments
    ///
    /// * `state` - Application state with access to NATS, database, and services
    /// * `consumer_name` - Unique consumer name for this worker instance
    /// * `cancel_token` - Token for graceful shutdown signaling
    /// * `config` - Worker configuration (concurrency limits, etc.)
    pub fn new(
        state: ServiceState,
        consumer_name: impl Into<String>,
        cancel_token: CancellationToken,
        config: &WorkerConfig,
    ) -> Self {
        Self {
            state,
            consumer_name: consumer_name.into(),
            cancel_token,
            semaphore: config.create_semaphore(),
        }
    }

    /// Spawns the worker as a background task.
    ///
    /// Returns a join handle that can be used to await worker completion
    /// or cancel it on shutdown.
    pub fn spawn(self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move { self.run().await })
    }

    /// Runs the worker loop, processing jobs as they arrive.
    #[tracing::instrument(
        skip(self),
        fields(consumer = %self.consumer_name),
        target = TRACING_TARGET,
        name = "processing_worker"
    )]
    async fn run(self) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting processing worker");

        let subscriber: DocumentJobSubscriber<ProcessingData> = self
            .state
            .nats
            .document_job_subscriber(&self.consumer_name)
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            consumer = %self.consumer_name,
            "Subscribed to processing jobs"
        );

        let mut stream = subscriber.subscribe().await?;

        loop {
            // Check for shutdown signal
            tokio::select! {
                biased;

                () = self.cancel_token.cancelled() => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        "Shutdown requested, stopping processing worker"
                    );
                    break;
                }

                result = stream.next() => {
                    let msg = match result {
                        Ok(Some(msg)) => msg,
                        Ok(None) => {
                            tracing::trace!(target: TRACING_TARGET, "No messages available");
                            continue;
                        }
                        Err(err) => {
                            tracing::error!(
                                target: TRACING_TARGET,
                                error = %err,
                                "Failed to receive message"
                            );
                            continue;
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
                            break;
                        }
                    };

                    let state = self.state.clone();
                    let job = msg.payload().clone();
                    let job_id = job.id;
                    let file_id = job.file_id;

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
                            stage = job.stage_name(),
                            task_count = job.data().tasks.len(),
                            "Processing document job"
                        );

                        match handle_job(&state, &job).await {
                            Ok(()) => {
                                tracing::info!(
                                    target: TRACING_TARGET,
                                    job_id = %job_id,
                                    file_id = %file_id,
                                    "Processing job completed"
                                );
                            }
                            Err(err) => {
                                tracing::error!(
                                    target: TRACING_TARGET,
                                    job_id = %job_id,
                                    file_id = %file_id,
                                    error = %err,
                                    "Processing job failed"
                                );
                                // TODO: Implement retry logic or dead letter queue
                            }
                        }
                    });
                }
            }
        }

        Ok(())
    }
}

/// Handles a single processing job.
#[tracing::instrument(
    skip(state, job),
    fields(
        job_id = %job.id,
        file_id = %job.file_id,
    ),
    target = TRACING_TARGET
)]
async fn handle_job(state: &ServiceState, job: &DocumentJob<ProcessingData>) -> Result<()> {
    let data = job.data();

    // TODO: Update database status to "processing"
    // let mut conn = state.postgres.get_connection().await?;
    // conn.update_file_status(job.file_id, ProcessingStatus::Processing).await?;

    // TODO: Fetch document from object store
    // let document_store = state.nats.document_store::<FilesBucket>().await?;
    // let document_key = DocumentKey::from_str(&job.storage_path)?;
    // let content = document_store.get(&document_key).await?;

    // Step 1: Process main prompt if provided
    if !data.prompt.is_empty() {
        tracing::debug!(
            target: TRACING_TARGET,
            prompt_length = data.prompt.len(),
            has_context = data.context.is_some(),
            "Executing VLM prompt"
        );
        // TODO: Implement VLM processing
        // - Send document + prompt to inference service
        // - Apply transformations based on VLM output
        let _inference = &state.inference;
    }

    // Step 2: Process annotations if specified
    if let Some(ref annotation_ids) = data.annotation_ids {
        tracing::debug!(
            target: TRACING_TARGET,
            annotation_count = annotation_ids.len(),
            "Processing annotations"
        );
        // TODO: Fetch annotations from database
        // TODO: Apply each annotation using VLM
    }

    // Step 3: Execute predefined tasks
    for task in &data.tasks {
        tracing::debug!(
            target: TRACING_TARGET,
            task = ?task,
            "Executing predefined task"
        );
        // TODO: Implement task execution
        // - Redact: Find and redact sensitive patterns
        // - Translate: Translate document to target language
        // - Summarize: Generate document summary
        // - ExtractInfo: Extract structured information
        // - etc.
    }

    // Step 4: Handle reference files if provided
    if let Some(ref reference_ids) = data.reference_file_ids {
        tracing::debug!(
            target: TRACING_TARGET,
            reference_count = reference_ids.len(),
            "Using reference files for context"
        );
        // TODO: Fetch reference files
        // TODO: Include in VLM context for style matching, etc.
    }

    // TODO: Store processed document back to object store
    // TODO: Update database status to "completed"
    // conn.update_file_status(job.file_id, ProcessingStatus::Completed).await?;

    Ok(())
}
