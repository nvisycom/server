//! Database models for all entities in the system.
//!
//! This module contains Diesel model definitions for all database tables,
//! including structs for querying, inserting, and updating records.

mod account;
mod account_action_token;
mod account_api_token;
mod account_notification;
mod document;
mod document_annotation;
mod document_chunk;
mod document_comment;
mod document_file;

mod workspace;
mod workspace_activity;
mod workspace_integration;
mod workspace_integration_run;
mod workspace_invite;
mod workspace_member;
mod workspace_webhook;

mod chat_operation;
mod chat_session;
mod chat_tool_call;

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
pub use document_chunk::{
    DocumentChunk, NewDocumentChunk, ScoredDocumentChunk, UpdateDocumentChunk,
};
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
// Chat models
pub use chat_operation::{ChatOperation, NewChatOperation, UpdateChatOperation};
pub use chat_session::{ChatSession, NewChatSession, UpdateChatSession};
pub use chat_tool_call::{ChatToolCall, NewChatToolCall, UpdateChatToolCall};
