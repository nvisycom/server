//! Preprocessing worker for document upload pipeline.
//!
//! Handles jobs triggered by file uploads:
//! - Format detection and validation
//! - Metadata extraction and fixes
//! - OCR for scanned documents
//! - Thumbnail generation
//! - Embedding generation for semantic search

use std::sync::Arc;

use nvisy_nats::stream::{DocumentJob, DocumentJobSubscriber, PreprocessingData};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::WorkerConfig;
use super::error::Result;
use crate::service::ServiceState;

/// Tracing target for preprocessing worker.
const TRACING_TARGET: &str = "nvisy_server::worker::preprocessing";

/// Background worker for preprocessing document jobs.
///
/// Subscribes to preprocessing stage jobs and processes them concurrently.
/// Each job runs format validation, OCR, thumbnail generation, and embedding
/// extraction based on the job configuration.
pub struct PreprocessingWorker {
    state: ServiceState,
    consumer_name: String,
    cancel_token: CancellationToken,
    semaphore: Arc<Semaphore>,
}

impl PreprocessingWorker {
    /// Creates a new preprocessing worker.
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
        name = "preprocessing_worker"
    )]
    async fn run(self) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting preprocessing worker");

        let subscriber: DocumentJobSubscriber<PreprocessingData> = self
            .state
            .nats
            .document_job_subscriber(&self.consumer_name)
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            consumer = %self.consumer_name,
            "Subscribed to preprocessing jobs"
        );

        let mut stream = subscriber.subscribe().await?;

        loop {
            // Check for shutdown signal
            tokio::select! {
                biased;

                () = self.cancel_token.cancelled() => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        "Shutdown requested, stopping preprocessing worker"
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

                    // Ack immediately to prevent redelivery while processing
                    // TODO: Consider acking after successful processing for at-least-once semantics
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
                            "Processing preprocessing job"
                        );

                        match handle_job(&state, &job).await {
                            Ok(()) => {
                                tracing::info!(
                                    target: TRACING_TARGET,
                                    job_id = %job_id,
                                    file_id = %file_id,
                                    "Preprocessing job completed"
                                );
                            }
                            Err(err) => {
                                tracing::error!(
                                    target: TRACING_TARGET,
                                    job_id = %job_id,
                                    file_id = %file_id,
                                    error = %err,
                                    "Preprocessing job failed"
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

/// Handles a single preprocessing job.
#[tracing::instrument(
    skip(state, job),
    fields(
        job_id = %job.id,
        file_id = %job.file_id,
    ),
    target = TRACING_TARGET
)]
async fn handle_job(state: &ServiceState, job: &DocumentJob<PreprocessingData>) -> Result<()> {
    let data = job.data();

    // TODO: Update database status to "processing"
    // let mut conn = state.postgres.get_connection().await?;
    // conn.update_file_status(job.file_id, ProcessingStatus::Processing).await?;

    // Step 1: Validate metadata
    if data.validate_metadata {
        tracing::debug!(target: TRACING_TARGET, "Validating file metadata");
        // TODO: Implement metadata validation
        // - Format detection
        // - File integrity checks
        // - Metadata extraction and fixes
    }

    // Step 2: Run OCR
    if data.run_ocr {
        tracing::debug!(target: TRACING_TARGET, "Running OCR");
        // TODO: Implement OCR
        // - Detect if document needs OCR (scanned vs native text)
        // - Extract text using OCR service
        // - Store extracted text in database
    }

    // Step 3: Generate embeddings
    if data.generate_embeddings {
        tracing::debug!(target: TRACING_TARGET, "Generating embeddings");
        // TODO: Implement embedding generation
        // - Split document into chunks
        // - Generate embeddings using inference service
        // - Store embeddings for semantic search
        let _inference = &state.inference;
    }

    // Step 4: Generate thumbnails
    if let Some(true) = data.generate_thumbnails {
        tracing::debug!(target: TRACING_TARGET, "Generating thumbnails");
        // TODO: Implement thumbnail generation
        // - Render first page(s) as images
        // - Store thumbnails in object store
    }

    // TODO: Update database status to "completed"
    // conn.update_file_status(job.file_id, ProcessingStatus::Completed).await?;

    Ok(())
}
