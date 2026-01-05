//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities for:
//!
//! - Document processing jobs
//! - Workspace event jobs

// Base types
mod event;
mod publisher;
mod subscriber;

// Document job
mod document_job;
mod document_job_pub;
mod document_job_sub;
mod document_task;

// Workspace event
mod workspace_event;
mod workspace_event_pub;
mod workspace_event_sub;

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
pub use workspace_event::{
    DocumentCreatedEvent, DocumentDeletedEvent, DocumentUpdateEvent, ErrorEvent,
    FilePostprocessedEvent, FilePreprocessedEvent, FileTransformedEvent, JobCompletedEvent,
    JobFailedEvent, JobProgressEvent, JobStage, JoinEvent, LeaveEvent, MemberAddedEvent,
    MemberPresenceEvent, MemberRemovedEvent, PostprocessingType, PreprocessingType,
    TransformationType, TypingEvent, WorkspaceEvent, WorkspaceUpdatedEvent, WorkspaceWsMessage,
};
pub use workspace_event_pub::WorkspaceEventPublisher;
pub use workspace_event_sub::{
    WorkspaceEventBatchStream, WorkspaceEventMessage, WorkspaceEventStream,
    WorkspaceEventSubscriber,
};
