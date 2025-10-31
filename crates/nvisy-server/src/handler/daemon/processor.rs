//! Simple file processor for handling document file processing pipeline.
//!
//! This module implements a basic file processing loop that reads files from
//! InputFiles storage, processes them, and stores results in IntermediateFiles storage.

use std::str::FromStr;
use std::time::Duration;

use nvisy_nats::NatsClient;
use nvisy_nats::object::{DocumentFileStore, InputFiles, IntermediateFiles, ObjectKey};
use nvisy_postgres::PgClient;
use nvisy_postgres::types::ProcessingStatus;
use tokio::time::sleep;
use tracing::{error, info};
use uuid::Uuid;

const TRACING_TARGET: &str = "nvisy_server::daemon::processor";

/// Simple file processor that runs in a loop.
pub struct FileProcessor {
    nats_client: NatsClient,
    pg_client: PgClient,
}

impl FileProcessor {
    /// Creates a new file processor instance.
    pub fn new(nats_client: NatsClient, pg_client: PgClient) -> Self {
        Self {
            nats_client,
            pg_client,
        }
    }

    /// Starts processing files in a loop.
    /// This function runs indefinitely until the task is cancelled.
    pub async fn start_processing_loop(&self) -> Result<(), ProcessorError> {
        info!(target: TRACING_TARGET, "Starting file processing loop");

        loop {
            if let Err(err) = self.process_pending_files().await {
                error!(
                    target: TRACING_TARGET,
                    error = %err,
                    "Error during file processing cycle"
                );
            }

            // Wait before next processing cycle
            sleep(Duration::from_secs(10)).await;
        }
    }

    /// Processes a single cycle of pending files.
    pub async fn process_pending_files(&self) -> Result<(), ProcessorError> {
        let input_store = self.nats_client.document_store::<InputFiles>().await?;
        let intermediate_store = self
            .nats_client
            .document_store::<IntermediateFiles>()
            .await?;
        let mut conn = self.pg_client.get_connection().await?;

        // Get pending files from database (placeholder - would need actual repository method)
        // For now, we'll just log that we're checking for files
        info!(target: TRACING_TARGET, "Checking for pending files to process");

        // In a real implementation, this would:
        // 1. Query database for files with ProcessingStatus::Pending
        // 2. Process each file
        // 3. Update status to ProcessingStatus::Completed or Failed

        // Placeholder implementation - in reality you'd get actual pending files
        let pending_files: Vec<nvisy_postgres::model::DocumentFile> = vec![];

        for file in pending_files {
            info!(
                target: TRACING_TARGET,
                file_id = %file.id,
                filename = %file.display_name,
                "Processing file"
            );

            match self
                .process_single_file(&input_store, &intermediate_store, &file)
                .await
            {
                Ok(()) => {
                    info!(
                        target: TRACING_TARGET,
                        file_id = %file.id,
                        "File processed successfully"
                    );

                    // Update status to completed (placeholder)
                    if let Err(err) = self
                        .update_file_status(&mut conn, file.id, ProcessingStatus::Completed, None)
                        .await
                    {
                        error!(
                            target: TRACING_TARGET,
                            error = %err,
                            file_id = %file.id,
                            "Failed to update file status to completed"
                        );
                    }
                }
                Err(err) => {
                    error!(
                        target: TRACING_TARGET,
                        error = %err,
                        file_id = %file.id,
                        "Failed to process file"
                    );

                    // Update status to failed
                    if let Err(update_err) = self
                        .update_file_status(
                            &mut conn,
                            file.id,
                            ProcessingStatus::Failed,
                            Some(err.to_string()),
                        )
                        .await
                    {
                        error!(
                            target: TRACING_TARGET,
                            error = %update_err,
                            file_id = %file.id,
                            "Failed to update file status after processing error"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Processes a single file from InputFiles to IntermediateFiles.
    async fn process_single_file(
        &self,
        input_store: &DocumentFileStore<InputFiles>,
        intermediate_store: &DocumentFileStore<IntermediateFiles>,
        file: &nvisy_postgres::model::DocumentFile,
    ) -> Result<(), ProcessorError> {
        // Parse the storage path to get the object key
        let input_key = ObjectKey::<InputFiles>::from_str(&file.storage_path).map_err(|err| {
            ProcessorError::InvalidStoragePath {
                path: file.storage_path.clone(),
                source: Box::new(err),
            }
        })?;

        // Retrieve the file content from InputFiles storage
        let content_data =
            input_store
                .get(&input_key)
                .await?
                .ok_or_else(|| ProcessorError::FileNotFound {
                    file_id: file.id,
                    storage_path: file.storage_path.clone(),
                })?;

        info!(
            target: TRACING_TARGET,
            file_id = %file.id,
            size = content_data.size(),
            "Retrieved file content for processing"
        );

        // Process the file content
        let processed_content = self.transform_content(&content_data, file).await?;

        // Create intermediate storage key
        let intermediate_key =
            intermediate_store.create_key(file.account_id, file.document_id, file.id);

        // Store processed content in IntermediateFiles storage
        let put_result = intermediate_store
            .put(&intermediate_key, &processed_content)
            .await?;

        info!(
            target: TRACING_TARGET,
            file_id = %file.id,
            intermediate_key = %intermediate_key.as_str(),
            size = put_result.size,
            "Stored processed content in intermediate storage"
        );

        Ok(())
    }

    /// Transforms/processes the file content.
    async fn transform_content(
        &self,
        content_data: &nvisy_nats::object::ContentData,
        file: &nvisy_postgres::model::DocumentFile,
    ) -> Result<nvisy_nats::object::ContentData, ProcessorError> {
        info!(
            target: TRACING_TARGET,
            file_id = %file.id,
            file_extension = %file.file_extension,
            file_size = file.file_size_bytes,
            "Processing file content"
        );

        // Simple processing - add metadata about processing
        let processed_data = if content_data.is_likely_text() {
            let text_content = String::from_utf8_lossy(content_data.as_bytes());
            let processed_text = format!(
                "PROCESSED FILE: {}\nOriginal Size: {} bytes\nProcessed At: {}\n\n--- ORIGINAL CONTENT ---\n{}",
                file.display_name,
                content_data.size(),
                time::OffsetDateTime::now_utc(),
                text_content
            );
            processed_text.into_bytes()
        } else {
            // For binary files, prepend processing metadata
            let mut processed = Vec::new();
            let header = format!(
                "PROCESSED_BINARY_FILE\nName: {}\nSize: {} bytes\nProcessed: {}\n---DATA---\n",
                file.display_name,
                content_data.size(),
                time::OffsetDateTime::now_utc()
            );
            processed.extend_from_slice(header.as_bytes());
            processed.extend_from_slice(content_data.as_bytes());
            processed
        };

        // Create content data with metadata
        let processed_content =
            DocumentFileStore::<IntermediateFiles>::create_content_data_with_metadata(
                processed_data.into(),
            );

        Ok(processed_content)
    }

    /// Updates file processing status in database.
    async fn update_file_status(
        &self,
        _conn: &mut nvisy_postgres::PgConnection,
        file_id: Uuid,
        status: ProcessingStatus,
        error_message: Option<String>,
    ) -> Result<(), ProcessorError> {
        // Placeholder implementation - in reality this would update the database
        info!(
            target: TRACING_TARGET,
            file_id = %file_id,
            status = ?status,
            error = ?error_message,
            "Would update file status in database"
        );

        // In real implementation:
        // let update = UpdateDocumentFile {
        //     processing_status: Some(status),
        //     processing_error: error_message,
        //     ..Default::default()
        // };
        // DocumentFileRepository::update_document_file(conn, file_id, update).await?;

        Ok(())
    }
}

/// Errors that can occur during file processing.
#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
    /// NATS operation failed
    #[error("NATS operation failed: {0}")]
    Nats(#[from] nvisy_nats::Error),

    /// Database operation failed
    #[error("Database operation failed: {0}")]
    Database(#[from] nvisy_postgres::PgError),

    /// File not found in storage
    #[error("File {file_id} not found at storage path: {storage_path}")]
    FileNotFound { file_id: Uuid, storage_path: String },

    /// Invalid storage path format
    #[error("Invalid storage path format: {path}")]
    InvalidStoragePath {
        path: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Processing failed
    #[error("File processing failed: {message}")]
    ProcessingFailed { message: String },
}

/// Example of how to use the processor in your application.
///
/// ```rust,no_run
/// use nvisy_server::handler::daemon::FileProcessor;
/// use nvisy_server::service::{ServiceConfig, ServiceState};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Initialize service state
///     let config = ServiceConfig::default();
///     let nats_client = config.connect_nats().await?;
///     let pg_client = config.connect_postgres().await?;
///
///     // Create and start the processor
///     let processor = FileProcessor::new(nats_client, pg_client);
///
///     // Start processing in a background task
///     tokio::spawn(async move {
///         if let Err(err) = processor.start_processing_loop().await {
///             eprintln!("Processor error: {}", err);
///         }
///     });
///
///     // Your main application logic here...
///     Ok(())
/// }
/// ```
///
/// ## Integration with Main Application
///
/// Here's how you would typically integrate this into your main server:
///
/// ```rust,no_run
/// use nvisy_server::handler::daemon::spawn_processor;
/// use nvisy_server::service::{ServiceConfig, ServiceState};
/// use axum::Router;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Initialize tracing
///     tracing_subscriber::fmt::init();
///
///     // Load configuration
///     let config = ServiceConfig::default();
///     let state = ServiceState::from_config(&config).await?;
///
///     // Start the file processor in background
///     let nats_client = config.connect_nats().await?;
///     let pg_client = config.connect_postgres().await?;
///     let _processor_handle = spawn_processor(nats_client, pg_client);
///
///     // Create your main application router
///     let app = Router::new()
///         .route("/health", axum::routing::get(|| async { "OK" }))
///         // Add your other routes here
///         .with_state(state);
///
///     // Start the web server
///     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
///     tracing::info!("Server listening on 0.0.0.0:3000");
///
///     axum::serve(listener, app).await?;
///     Ok(())
/// }
/// ```
pub fn spawn_processor(
    nats_client: NatsClient,
    pg_client: PgClient,
) -> tokio::task::JoinHandle<()> {
    let processor = FileProcessor::new(nats_client, pg_client);

    tokio::spawn(async move {
        if let Err(err) = processor.start_processing_loop().await {
            error!(target: TRACING_TARGET, error = %err, "File processor loop terminated with error");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_error_display() {
        let error = ProcessorError::FileNotFound {
            file_id: Uuid::new_v4(),
            storage_path: "test/path".to_string(),
        };
        let display = format!("{}", error);
        assert!(display.contains("File"));
        assert!(display.contains("not found"));
    }

    #[test]
    fn test_processing_failed_error() {
        let error = ProcessorError::ProcessingFailed {
            message: "Test error".to_string(),
        };
        assert_eq!(error.to_string(), "File processing failed: Test error");
    }
}
