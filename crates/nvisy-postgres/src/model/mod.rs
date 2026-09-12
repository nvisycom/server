//! Database models for all entities in the system.
//!
//! This module contains Diesel model definitions for all database tables,
//! including structs for querying, inserting, and updating records.

mod account_api_tokens;
mod account_identities;
mod account_notifications;
mod accounts;
mod pipeline_references;
mod workspace_activities;
mod workspace_assistant_jobs;
mod workspace_audits;
mod workspace_blobs;
mod workspace_connection_schedule;
mod workspace_connection_syncs;
mod workspace_connections;
mod workspace_detection_jobs;
mod workspace_detection_policy_versions;
mod workspace_detection_usage;
mod workspace_detections;
mod workspace_document_exports;
mod workspace_document_imports;
mod workspace_documents;
mod workspace_event_outbox;
mod workspace_invites;
mod workspace_members;
mod workspace_pipelines;
mod workspace_policies;
mod workspace_policy_versions;
mod workspace_providers;
mod workspace_redactions;
mod workspace_thread_comments;
mod workspace_thread_events;
mod workspace_threads;
mod workspace_webhooks;
mod workspaces;

// Account models
pub use account_api_tokens::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
pub use account_identities::{AccountIdentity, NewAccountIdentity};
pub use account_notifications::{
    AccountNotification, NewAccountNotification, UpdateAccountNotification,
};
pub use accounts::{Account, NewAccount, UpdateAccount};
pub use pipeline_references::PipelinePolicy;
pub use workspace_activities::{NewWorkspaceActivity, WorkspaceActivity};
pub use workspace_assistant_jobs::{NewWorkspaceAssistantJob, WorkspaceAssistantJob};
pub use workspace_audits::{NewWorkspaceAudit, WorkspaceAudit};
pub use workspace_blobs::{Blob, NewBlob};
pub use workspace_connection_schedule::{
    NewWorkspaceConnectionSchedule, WorkspaceConnectionSchedule,
};
pub use workspace_connection_syncs::{
    NewWorkspaceConnectionSync, UpdateWorkspaceConnectionSync, WorkspaceConnectionSync,
};
pub use workspace_connections::{
    NewWorkspaceConnection, UpdateWorkspaceConnection, WorkspaceConnection,
};
pub use workspace_detection_jobs::{NewWorkspaceDetectionJob, WorkspaceDetectionJob};
pub use workspace_detection_policy_versions::DetectionPolicyVersion;
pub use workspace_detection_usage::{NewWorkspaceDetectionUsage, WorkspaceDetectionUsage};
// Detection / pipeline models
pub use workspace_detections::{
    NewWorkspaceDetection, UpdateWorkspaceDetection, WorkspaceDetection,
};
pub use workspace_document_exports::{NewWorkspaceDocumentExport, WorkspaceDocumentExport};
pub use workspace_document_imports::{NewWorkspaceDocumentImport, WorkspaceDocumentImport};
pub use workspace_documents::{NewWorkspaceDocument, UpdateWorkspaceDocument, WorkspaceDocument};
pub use workspace_event_outbox::{NewWorkspaceEventOutbox, WorkspaceEventOutbox};
pub use workspace_invites::{NewWorkspaceInvite, UpdateWorkspaceInvite, WorkspaceInvite};
pub use workspace_members::{NewWorkspaceMember, UpdateWorkspaceMember, WorkspaceMember};
pub use workspace_pipelines::{NewWorkspacePipeline, UpdateWorkspacePipeline, WorkspacePipeline};
pub use workspace_policies::{NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy};
pub use workspace_policy_versions::{NewWorkspacePolicyVersion, WorkspacePolicyVersion};
pub use workspace_providers::{NewWorkspaceProvider, UpdateWorkspaceProvider, WorkspaceProvider};
pub use workspace_redactions::{NewWorkspaceRedaction, WorkspaceRedaction};
pub use workspace_thread_comments::{
    NewWorkspaceThreadComment, UpdateWorkspaceThreadComment, WorkspaceThreadComment,
};
pub use workspace_thread_events::{NewWorkspaceThreadEvent, WorkspaceThreadEvent};
pub use workspace_threads::{NewWorkspaceThread, UpdateWorkspaceThread, WorkspaceThread};
pub use workspace_webhooks::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
// Workspace models
pub use workspaces::{NewWorkspace, UpdateWorkspace, Workspace};
