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
