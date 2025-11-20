//! Project request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request payload for creating a new project.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "My Project",
    "description": "A project for document processing",
    "keepForSec": 86400,
    "autoCleanup": true,
    "requireApproval": false
}))]
pub struct CreateProject {
    /// Display name of the project.
    #[validate(length(min = 3, max = 100))]
    pub display_name: String,

    /// Description of the project.
    #[validate(length(min = 1, max = 200))]
    pub description: Option<String>,

    /// Duration in seconds to keep the original files.
    #[validate(range(min = 60, max = 604800))]
    pub keep_for_sec: Option<i32>,

    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: Option<bool>,

    /// Whether approval is required to processed files to be visible.
    pub require_approval: Option<bool>,

    /// Maximum number of members allowed in the project.
    #[validate(range(min = 1, max = 1000))]
    pub max_members: Option<i32>,

    /// Maximum storage size in megabytes allowed for the project.
    #[validate(range(min = 1, max = 1048576))]
    pub max_storage: Option<i32>,

    /// Whether comments are enabled for this project.
    pub enable_comments: Option<bool>,
}

/// Request payload for project archival or restoration.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "reason": "Project completed",
    "notifyMembers": true
}))]
pub struct ArchiveProject {
    /// Reason for archiving.
    #[validate(length(min = 1, max = 200))]
    pub reason: String,
}

/// Request payload to update project.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Updated Project Name",
    "description": "Updated description"
}))]
pub struct UpdateProject {
    /// Display name of the project.
    #[validate(length(min = 3, max = 100))]
    pub display_name: Option<String>,

    /// Description of the project.
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// Duration in seconds to keep the original files.
    #[validate(range(min = 60, max = 604800))]
    pub keep_for_sec: Option<i32>,

    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: Option<bool>,

    /// Whether approval is required to processed files to be visible.
    pub require_approval: Option<bool>,

    /// Maximum number of members allowed in the project.
    #[validate(range(min = 1, max = 1000))]
    pub max_members: Option<i32>,

    /// Maximum storage size in megabytes allowed for the project.
    #[validate(range(min = 1, max = 1048576))]
    pub max_storage: Option<i32>,

    /// Whether comments are enabled for this project.
    pub enable_comments: Option<bool>,
}
