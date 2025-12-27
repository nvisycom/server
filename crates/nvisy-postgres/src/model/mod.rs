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
pub mod document_comment;
pub mod document_file;

pub mod project;
pub mod project_activity;
pub mod project_integration;
pub mod project_invite;
pub mod project_member;
pub mod project_pipeline;
pub mod project_run;
pub mod project_template;
pub mod project_webhook;

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
pub use document_comment::{DocumentComment, NewDocumentComment, UpdateDocumentComment};
pub use document_file::{DocumentFile, NewDocumentFile, UpdateDocumentFile};
// Project models
pub use project::{NewProject, Project, UpdateProject};
pub use project_activity::{NewProjectActivity, ProjectActivity};
pub use project_integration::{
    NewProjectIntegration, ProjectIntegration, UpdateProjectIntegration,
};
pub use project_invite::{NewProjectInvite, ProjectInvite, UpdateProjectInvite};
pub use project_member::{NewProjectMember, ProjectMember, UpdateProjectMember};
pub use project_pipeline::{NewProjectPipeline, ProjectPipeline, UpdateProjectPipeline};
pub use project_run::{NewProjectRun, ProjectRun, UpdateProjectRun};
pub use project_template::{NewProjectTemplate, ProjectTemplate, UpdateProjectTemplate};
pub use project_webhook::{NewProjectWebhook, ProjectWebhook, UpdateProjectWebhook};
