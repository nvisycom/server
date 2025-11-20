//! Document version request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new document version.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "versionName": "v2.0",
    "description": "Major update with new features",
    "basedOn": "550e8400-e29b-41d4-a716-446655440000",
    "autoIncrement": false
}))]
pub struct CreateDocumentVersion {
    /// Version name/label.
    #[validate(length(min = 1, max = 50))]
    pub version_name: Option<String>,

    /// Description of changes in this version.
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// Base version to create this version from.
    pub based_on: Option<Uuid>,

    /// Whether to auto-increment version number.
    pub auto_increment: Option<bool>,
}

/// Request payload for updating a document version.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "versionName": "v2.1",
    "description": "Bug fixes and improvements",
    "isPublished": true
}))]
pub struct UpdateDocumentVersion {
    /// Updated version name/label.
    #[validate(length(min = 1, max = 50))]
    pub version_name: Option<String>,

    /// Updated description.
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// Whether this version is published/visible.
    pub is_published: Option<bool>,
}

/// Request payload for comparing document versions.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "sourceVersionId": "550e8400-e29b-41d4-a716-446655440000",
    "targetVersionId": "550e8400-e29b-41d4-a716-446655440001",
    "compareOptions": {
        "showWhitespace": false,
        "ignoreCase": false
    }
}))]
pub struct CompareDocumentVersions {
    /// Source version for comparison.
    pub source_version_id: Uuid,

    /// Target version for comparison.
    pub target_version_id: Uuid,
}

/// Request payload for restoring a document version.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "reason": "Reverting to stable version",
    "createBackup": true,
    "notifyMembers": false
}))]
pub struct RestoreDocumentVersion {
    /// Reason for restoration.
    #[validate(length(min = 1, max = 200))]
    pub reason: String,

    /// Whether to create a backup before restoring.
    pub create_backup: Option<bool>,

    /// Whether to notify project members.
    pub notify_members: Option<bool>,
}
