//! Database models for all entities in the system.
//!
//! This module contains Diesel model definitions for all database tables,
//! including structs for querying, inserting, and updating records.

pub mod account;
pub mod account_action_token;
pub mod account_api_token;
pub mod account_notification;
pub mod document;
pub mod document_annotation;
pub mod document_chunk;
pub mod document_comment;
pub mod document_file;

pub mod workspace;
pub mod workspace_activity;
pub mod workspace_integration;
pub mod workspace_integration_run;
pub mod workspace_invite;
pub mod workspace_member;
pub mod workspace_webhook;

// Account models
pub use account::{Account, NewAccount, UpdateAccount};
pub use account_action_token::{
    AccountActionToken, NewAccountActionToken, UpdateAccountActionToken,
};
pub use account_api_token::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
pub use account_notification::{
    AccountNotification, NewAccountNotification, UpdateAccountNotification,
};
// Document models
pub use document::{Document, NewDocument, UpdateDocument};
pub use document_annotation::{
    DocumentAnnotation, NewDocumentAnnotation, UpdateDocumentAnnotation,
};
pub use document_chunk::{DocumentChunk, NewDocumentChunk, UpdateDocumentChunk};
pub use document_comment::{DocumentComment, NewDocumentComment, UpdateDocumentComment};
pub use document_file::{DocumentFile, NewDocumentFile, UpdateDocumentFile};
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
