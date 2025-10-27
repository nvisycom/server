//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities for:
//!
//! - Document processing jobs
//! - Project import/export jobs
//! - Project event jobs

// Base types
mod event;
mod publisher;
mod subscriber;

// Document job
mod document_job;
mod document_job_pub;
mod document_job_sub;

// Project import
mod project_import;
mod project_import_pub;
mod project_import_sub;

// Project export
mod project_export;
mod project_export_pub;
mod project_export_sub;

// Project event
mod project_event;
mod project_event_pub;
mod project_event_sub;

// Re-export event types
// Re-export document job types
pub use document_job::{
    DocumentJob, DocumentJobPayload, ProcessingOptions, ProcessingStage, ProcessingType,
};
pub use document_job_pub::DocumentJobPublisher;
pub use document_job_sub::{
    DocumentJobBatchStream, DocumentJobMessage, DocumentJobStream, DocumentJobSubscriber,
};
pub use event::{
    CancelledStatus, CompletedStatus, EventPriority, EventStatus, FailedStatus, ProcessingStatus,
};
// Re-export project event types (WebSocket real-time communication)
pub use project_event::{
    DocumentCreatedEvent, DocumentDeletedEvent, DocumentUpdateEvent, ErrorEvent,
    FileProcessedEvent, FileRedactedEvent, FileVerifiedEvent, JoinEvent, LeaveEvent,
    MemberAddedEvent, MemberPresenceEvent, MemberRemovedEvent, ProjectEvent, ProjectUpdatedEvent,
    ProjectWsMessage, TypingEvent,
};
pub use project_event_pub::ProjectEventPublisher;
pub use project_event_sub::{
    ProjectEventBatchStream, ProjectEventMessage, ProjectEventStream, ProjectEventSubscriber,
};
// Re-export project export types
pub use project_export::{ProjectExportJob, ProjectExportPayload};
pub use project_export_pub::ProjectExportPublisher;
pub use project_export_sub::{
    ProjectExportBatchStream, ProjectExportMessage, ProjectExportStream, ProjectExportSubscriber,
};
// Re-export project import types
pub use project_import::{ProjectImportJob, ProjectImportPayload};
pub use project_import_pub::ProjectImportPublisher;
pub use project_import_sub::{
    ProjectImportBatchStream, ProjectImportMessage, ProjectImportStream, ProjectImportSubscriber,
};
// Re-export base publisher/subscriber types
pub use publisher::StreamPublisher;
pub use subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
