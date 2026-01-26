//! Database models for all entities in the system.
//!
//! This module contains Diesel model definitions for all database tables,
//! including structs for querying, inserting, and updating records.

mod account;
mod account_action_token;
mod account_api_token;
mod account_notification;
mod file;
mod file_annotation;
mod file_chunk;
mod pipeline;
mod pipeline_run;

mod workspace;
mod workspace_activity;
mod workspace_integration;
mod workspace_integration_run;
mod workspace_invite;
mod workspace_member;
mod workspace_webhook;

// Account models
pub use account::{Account, NewAccount, UpdateAccount};
pub use account_action_token::{
    AccountActionToken, NewAccountActionToken, UpdateAccountActionToken,
};
pub use account_api_token::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
pub use account_notification::{
    AccountNotification, NewAccountNotification, UpdateAccountNotification,
};
// File models
pub use file::{File, NewFile, UpdateFile};
pub use file_annotation::{FileAnnotation, NewFileAnnotation, UpdateFileAnnotation};
pub use file_chunk::{FileChunk, NewFileChunk, ScoredFileChunk, UpdateFileChunk};
// Pipeline models
pub use pipeline::{NewPipeline, Pipeline, UpdatePipeline};
pub use pipeline_run::{NewPipelineRun, PipelineRun, UpdatePipelineRun};
// Workspace models
pub use workspace::{NewWorkspace, UpdateWorkspace, Workspace};
pub use workspace_activity::{NewWorkspaceActivity, WorkspaceActivity};
pub use workspace_integration::{
    NewWorkspaceIntegration, UpdateWorkspaceIntegration, WorkspaceIntegration,
};
pub use workspace_integration_run::{
    NewWorkspaceIntegrationRun, UpdateWorkspaceIntegrationRun, WorkspaceIntegrationRun,
};
pub use workspace_invite::{NewWorkspaceInvite, UpdateWorkspaceInvite, WorkspaceInvite};
pub use workspace_member::{NewWorkspaceMember, UpdateWorkspaceMember, WorkspaceMember};
pub use workspace_webhook::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
