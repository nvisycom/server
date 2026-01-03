//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities for:
//!
//! - Document processing jobs
//! - Workspace import/export jobs
//! - Workspace event jobs

// Base types
mod event;
mod publisher;
mod subscriber;

// Document job
mod document_job;
mod document_job_pub;
mod document_job_sub;

// Workspace import
mod workspace_import;
mod workspace_import_pub;
mod workspace_import_sub;

// Workspace export
mod workspace_export;
mod workspace_export_pub;
mod workspace_export_sub;

// Workspace event
mod workspace_event;
mod workspace_event_pub;
mod workspace_event_sub;

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
// Re-export base publisher/subscriber types
pub use publisher::StreamPublisher;
pub use subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};
// Re-export workspace event types (WebSocket real-time communication)
pub use workspace_event::{
    DocumentCreatedEvent, DocumentDeletedEvent, DocumentUpdateEvent, ErrorEvent,
    FileProcessedEvent, FileRedactedEvent, FileVerifiedEvent, JoinEvent, LeaveEvent,
    MemberAddedEvent, MemberPresenceEvent, MemberRemovedEvent, TypingEvent, WorkspaceEvent,
    WorkspaceUpdatedEvent, WorkspaceWsMessage,
};
pub use workspace_event_pub::WorkspaceEventPublisher;
pub use workspace_event_sub::{
    WorkspaceEventBatchStream, WorkspaceEventMessage, WorkspaceEventStream,
    WorkspaceEventSubscriber,
};
// Re-export workspace export types
pub use workspace_export::{WorkspaceExportJob, WorkspaceExportPayload};
pub use workspace_export_pub::WorkspaceExportPublisher;
pub use workspace_export_sub::{
    WorkspaceExportBatchStream, WorkspaceExportMessage, WorkspaceExportStream,
    WorkspaceExportSubscriber,
};
// Re-export workspace import types
pub use workspace_import::{WorkspaceImportJob, WorkspaceImportPayload};
pub use workspace_import_pub::WorkspaceImportPublisher;
pub use workspace_import_sub::{
    WorkspaceImportBatchStream, WorkspaceImportMessage, WorkspaceImportStream,
    WorkspaceImportSubscriber,
};
