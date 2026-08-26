//! Database models for all entities in the system.
//!
//! This module contains Diesel model definitions for all database tables,
//! including structs for querying, inserting, and updating records.

mod account;
mod account_api_token;
mod account_notification;
mod chat_message;
mod chat_session;
mod event_outbox;
mod pipeline_reference;
mod workspace;
mod workspace_activity;
mod workspace_connection;
mod workspace_connection_schedule;
mod workspace_connection_sync;
mod workspace_detection;
mod workspace_detection_usage;
mod workspace_file;
mod workspace_file_imports;
mod workspace_invite;
mod workspace_member;
mod workspace_pipeline;
mod workspace_policy;
mod workspace_redaction;
mod workspace_webhook;

// Account models
pub use account::{Account, NewAccount, UpdateAccount};
pub use account_api_token::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
pub use account_notification::{
    AccountNotification, NewAccountNotification, UpdateAccountNotification,
};
// Chat models
pub use chat_message::{ChatMessage, NewChatMessage};
pub use chat_session::{ChatSession, NewChatSession, UpdateChatSession};
pub use event_outbox::{EventOutbox, NewEventOutbox};
pub use pipeline_reference::PipelinePolicy;
// Workspace models
pub use workspace::{NewWorkspace, UpdateWorkspace, Workspace};
pub use workspace_activity::{NewWorkspaceActivity, WorkspaceActivity};
pub use workspace_connection::{
    NewWorkspaceConnection, UpdateWorkspaceConnection, WorkspaceConnection,
};
pub use workspace_connection_schedule::{
    NewWorkspaceConnectionSchedule, WorkspaceConnectionSchedule,
};
pub use workspace_connection_sync::{
    NewWorkspaceConnectionSync, UpdateWorkspaceConnectionSync, WorkspaceConnectionSync,
};
// Detection / pipeline models
pub use workspace_detection::{
    NewWorkspaceDetection, UpdateWorkspaceDetection, WorkspaceDetection,
};
pub use workspace_detection_usage::{NewWorkspaceDetectionUsage, WorkspaceDetectionUsage};
pub use workspace_file::{NewWorkspaceFile, UpdateWorkspaceFile, WorkspaceFile};
pub use workspace_file_imports::{NewWorkspaceFileImport, WorkspaceFileImport};
pub use workspace_invite::{NewWorkspaceInvite, UpdateWorkspaceInvite, WorkspaceInvite};
pub use workspace_member::{NewWorkspaceMember, UpdateWorkspaceMember, WorkspaceMember};
pub use workspace_pipeline::{NewWorkspacePipeline, UpdateWorkspacePipeline, WorkspacePipeline};
pub use workspace_policy::{NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy};
pub use workspace_redaction::{NewWorkspaceRedaction, WorkspaceRedaction};
pub use workspace_webhook::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
