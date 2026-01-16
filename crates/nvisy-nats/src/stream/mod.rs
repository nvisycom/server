//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities for:
//!
//! - Document processing jobs

// Base types
mod event;
mod publisher;
mod subscriber;

// Document job
mod document_job;
mod document_job_pub;
mod document_job_sub;
mod document_task;

pub use document_job::{
    CompressionLevel, DocumentJob, PostprocessingData, PreprocessingData, ProcessingData,
    ProcessingQuality, STREAM_NAME as DOCUMENT_JOB_STREAM, Stage,
};
pub use document_job_pub::DocumentJobPublisher;
pub use document_job_sub::DocumentJobSubscriber;
pub use document_task::{GenerateInfoType, InsertValue, MergeOrder, PredefinedTask, SplitStrategy};
pub use event::EventPriority;
pub use publisher::StreamPublisher;
pub use subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
