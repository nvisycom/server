//! Simple file processor for background processing.
//!
//! This module provides a basic file processor that reads files from InputFiles
//! storage, processes them, and stores results in IntermediateFiles storage using
//! NATS document store.
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use nvisy_server::handler::daemon::{FileProcessor, spawn_processor};
//! use nvisy_server::service::{ServiceConfig, ServiceState};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize service clients
//!     let config = ServiceConfig::default();
//!     let nats_client = config.connect_nats().await?;
//!     let pg_client = config.connect_postgres().await?;
//!
//!     // Spawn the processor in the background
//!     let _processor_handle = spawn_processor(nats_client, pg_client);
//!
//!     // Your main application logic here...
//!     // The processor will run in the background processing files
//!
//!     Ok(())
//! }
//! ```
//!
//! The processor will continuously monitor for files with `ProcessingStatus::Pending`,
//! process them, and store the results in the IntermediateFiles document store.

pub mod processor;

pub use processor::{FileProcessor, ProcessorError, spawn_processor};
