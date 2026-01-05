//! Postprocessing worker for document download pipeline.
//!
//! Handles jobs triggered by download requests:
//! - Format conversion to requested format
//! - Compression settings
//! - Annotation flattening (burning into document)
//! - Cleanup of temporary artifacts

use std::sync::Arc;

use nvisy_nats::stream::{DocumentJob, DocumentJobSubscriber, PostprocessingData};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::service::WorkerState;

/// Tracing target for postprocessing worker.
const TRACING_TARGET: &str = "nvisy_worker::postprocessing";

/// Background worker for postprocessing document jobs.
///
/// Subscribes to postprocessing stage jobs and prepares documents
/// for download. Handles format conversion, compression, and
/// cleanup of temporary processing artifacts.
pub struct PostprocessingWorker {
    state: WorkerState,
    consumer_name: String,
    cancel_token: CancellationToken,
    semaphore: Arc<Semaphore>,
}

impl PostprocessingWorker {
    /// Creates a new postprocessing worker.
    ///
    /// # Arguments
    ///
    /// * `state` - Application state with access to NATS, database, and services
    /// * `consumer_name` - Unique consumer name for this worker instance
    /// * `cancel_token` - Token for graceful shutdown signaling
    pub fn new(
        state: WorkerState,
        consumer_name: impl Into<String>,
        cancel_token: CancellationToken,
    ) -> Self {
        let semaphore = state.create_semaphore();
        Self {
            state,
            consumer_name: consumer_name.into(),
            cancel_token,
            semaphore,
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
        name = "postprocessing_worker"
    )]
    async fn run(self) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting postprocessing worker");

        let subscriber: DocumentJobSubscriber<PostprocessingData> = self
            .state
            .nats
            .document_job_subscriber(&self.consumer_name)
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            consumer = %self.consumer_name,
            "Subscribed to postprocessing jobs"
        );

        let mut stream = subscriber.subscribe().await?;

        loop {
            // Check for shutdown signal
            tokio::select! {
                biased;

                () = self.cancel_token.cancelled() => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        "Shutdown requested, stopping postprocessing worker"
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
                            target_format = ?job.data().target_format,
                            "Processing postprocessing job"
                        );

                        match handle_job(&state, &job).await {
                            Ok(()) => {
                                tracing::info!(
                                    target: TRACING_TARGET,
                                    job_id = %job_id,
                                    file_id = %file_id,
                                    "Postprocessing job completed"
                                );
                            }
                            Err(err) => {
                                tracing::error!(
                                    target: TRACING_TARGET,
                                    job_id = %job_id,
                                    file_id = %file_id,
                                    error = %err,
                                    "Postprocessing job failed"
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

/// Handles a single postprocessing job.
#[tracing::instrument(
    skip(state, job),
    fields(
        job_id = %job.id,
        file_id = %job.file_id,
    ),
    target = TRACING_TARGET
)]
async fn handle_job(state: &WorkerState, job: &DocumentJob<PostprocessingData>) -> Result<()> {
    let data = job.data();
    let _ = state; // Suppress unused warning for now

    // TODO: Update database status to "processing"
    // let mut conn = state.postgres.get_connection().await?;
    // conn.update_file_status(job.file_id, ProcessingStatus::Processing).await?;

    // TODO: Fetch document from object store
    // let document_store = state.nats.document_store::<FilesBucket>().await?;
    // let document_key = DocumentKey::from_str(&job.storage_path)?;
    // let content = document_store.get(&document_key).await?;

    // Step 1: Flatten annotations if requested
    if let Some(true) = data.flatten_annotations {
        tracing::debug!(target: TRACING_TARGET, "Flattening annotations into document");
        // TODO: Burn annotations into document
        // - Fetch annotations from database
        // - Render them permanently into document
    }

    // Step 2: Convert format if specified
    if let Some(ref target_format) = data.target_format {
        tracing::debug!(
            target: TRACING_TARGET,
            target_format = %target_format,
            source_format = %job.file_extension,
            "Converting document format"
        );
        // TODO: Implement format conversion
        // - PDF <-> DOCX, PNG, etc.
        // - Use appropriate conversion libraries
    }

    // Step 3: Apply compression if specified
    if let Some(ref compression_level) = data.compression_level {
        tracing::debug!(
            target: TRACING_TARGET,
            compression_level = ?compression_level,
            "Applying compression"
        );
        // TODO: Implement compression
        // - Compress images in document
        // - Optimize file size based on level
    }

    // Step 4: Run cleanup tasks
    if let Some(ref cleanup_tasks) = data.cleanup_tasks {
        tracing::debug!(
            target: TRACING_TARGET,
            task_count = cleanup_tasks.len(),
            "Running cleanup tasks"
        );
        // TODO: Implement cleanup
        // - Remove temporary files
        // - Clean intermediate processing artifacts
    }

    // TODO: Store processed document to object store
    // TODO: Update database with final file info
    // TODO: Update database status to "completed"
    // conn.update_file_status(job.file_id, ProcessingStatus::Completed).await?;

    Ok(())
}
