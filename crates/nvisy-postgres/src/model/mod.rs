//! Database models for all entities in the system.
//!
//! This module contains Diesel model definitions for all database tables,
//! including structs for querying, inserting, and updating records.

mod account;
mod account_api_token;
mod account_notification;
mod pipeline_reference;
mod workspace;
mod workspace_activity;
mod workspace_connection;
mod workspace_connection_run;
mod workspace_context;
mod workspace_file;
mod workspace_invite;
mod workspace_member;
mod workspace_pipeline;
mod workspace_pipeline_artifact;
mod workspace_pipeline_run;
mod workspace_policy;
mod workspace_webhook;

// Account models
pub use account::{Account, NewAccount, UpdateAccount};
pub use account_api_token::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
pub use account_notification::{
    AccountNotification, NewAccountNotification, UpdateAccountNotification,
};
pub use pipeline_reference::{PipelineContext, PipelinePolicy};
// Workspace models
pub use workspace::{NewWorkspace, UpdateWorkspace, Workspace};
pub use workspace_activity::{NewWorkspaceActivity, WorkspaceActivity};
pub use workspace_connection::{
    NewWorkspaceConnection, UpdateWorkspaceConnection, WorkspaceConnection,
};
pub use workspace_connection_run::{
    NewWorkspaceConnectionRun, UpdateWorkspaceConnectionRun, WorkspaceConnectionRun,
};
pub use workspace_context::{NewWorkspaceContext, UpdateWorkspaceContext, WorkspaceContext};
pub use workspace_file::{NewWorkspaceFile, UpdateWorkspaceFile, WorkspaceFile};
pub use workspace_invite::{NewWorkspaceInvite, UpdateWorkspaceInvite, WorkspaceInvite};
pub use workspace_member::{NewWorkspaceMember, UpdateWorkspaceMember, WorkspaceMember};
pub use workspace_pipeline::{NewWorkspacePipeline, UpdateWorkspacePipeline, WorkspacePipeline};
// Pipeline models
pub use workspace_pipeline_artifact::{NewWorkspacePipelineArtifact, WorkspacePipelineArtifact};
pub use workspace_pipeline_run::{
    NewWorkspacePipelineRun, UpdateWorkspacePipelineRun, WorkspacePipelineRun,
};
pub use workspace_policy::{NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy};
pub use workspace_webhook::{NewWorkspaceWebhook, UpdateWorkspaceWebhook, WorkspaceWebhook};
