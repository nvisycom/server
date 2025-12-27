//! Project request types.
//!
//! This module provides request DTOs for project management operations including
//! creation, updates, and archival. All request types support JSON serialization
//! and validation.

use nvisy_postgres::model::{NewProject, UpdateProject as UpdateProjectModel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new project.
///
/// Creates a new project with the specified configuration. The creator is
/// automatically added as an admin member of the project.
///
/// # Example
///
/// ```json
/// {
///   "displayName": "My Project",
///   "description": "A sample project",
///   "keepForSec": 86400,
///   "autoCleanup": true
/// }
/// ```
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    /// Display name of the project (3-100 characters).
    #[validate(length(min = 3, max = 100))]
    pub display_name: String,

    /// Optional description of the project (max 200 characters).
    #[validate(length(max = 200))]
    pub description: Option<String>,

    /// Duration in seconds to keep the original files (60-604800 seconds).
    #[validate(range(min = 60, max = 604800))]
    pub keep_for_sec: Option<i32>,

    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: Option<bool>,

    /// Whether approval is required for processed files to be visible.
    pub require_approval: Option<bool>,

    /// Maximum number of members allowed in the project (1-1000).
    #[validate(range(min = 1, max = 1000))]
    pub max_members: Option<i32>,

    /// Maximum storage size in megabytes allowed for the project (1024-1048576 MB).
    #[validate(range(min = 1024, max = 1048576))]
    pub max_storage: Option<i32>,

    /// Whether comments are enabled for this project.
    pub enable_comments: Option<bool>,
}

impl CreateProject {
    /// Converts this request into a [`NewProject`] model for database insertion.
    ///
    /// # Arguments
    ///
    /// * `account_id` - The ID of the account creating the project (becomes the owner).
    #[inline]
    pub fn into_model(self, account_id: Uuid) -> NewProject {
        NewProject {
            display_name: self.display_name,
            description: self.description,
            keep_for_sec: self.keep_for_sec,
            auto_cleanup: self.auto_cleanup,
            require_approval: self.require_approval,
            max_members: self.max_members,
            max_storage: self.max_storage,
            enable_comments: self.enable_comments,
            created_by: account_id,
            ..Default::default()
        }
    }
}

/// Request payload for project archival or restoration.
///
/// Used when archiving a project to record the reason for the action.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveProject {
    /// Reason for archiving (1-200 characters).
    #[validate(length(min = 1, max = 200))]
    pub reason: String,
}

/// Request payload to update an existing project.
///
/// All fields are optional; only provided fields will be updated.
///
/// # Example
///
/// ```json
/// {
///   "displayName": "Updated Project Name",
///   "enableComments": true
/// }
/// ```
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    /// New display name for the project (3-100 characters).
    #[validate(length(min = 3, max = 100))]
    pub display_name: Option<String>,

    /// New description for the project (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// New duration in seconds to keep original files (60-604800 seconds).
    #[validate(range(min = 60, max = 604800))]
    pub keep_for_sec: Option<i32>,

    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: Option<bool>,

    /// Whether approval is required for processed files to be visible.
    pub require_approval: Option<bool>,

    /// Maximum number of members allowed in the project (1-1000).
    #[validate(range(min = 1, max = 1000))]
    pub max_members: Option<i32>,

    /// Maximum storage size in megabytes allowed for the project (1-1048576 MB).
    #[validate(range(min = 1, max = 1048576))]
    pub max_storage: Option<i32>,

    /// Whether comments are enabled for this project.
    pub enable_comments: Option<bool>,
}

impl UpdateProject {
    /// Converts this request into an [`UpdateProjectModel`] for database update.
    #[inline]
    pub fn into_model(self) -> UpdateProjectModel {
        UpdateProjectModel {
            display_name: self.display_name,
            description: self.description,
            keep_for_sec: self.keep_for_sec,
            auto_cleanup: self.auto_cleanup,
            require_approval: self.require_approval,
            max_members: self.max_members,
            max_storage: self.max_storage,
            enable_comments: self.enable_comments,
            ..Default::default()
        }
    }
}
