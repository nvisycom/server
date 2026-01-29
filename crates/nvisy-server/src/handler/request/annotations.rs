//! Annotation request types.

use nvisy_postgres::model::{NewWorkspaceFileAnnotation, UpdateWorkspaceFileAnnotation};
use nvisy_postgres::types::AnnotationType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request to create an annotation.
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAnnotation {
    /// Annotation content.
    #[validate(length(min = 1, max = 10000))]
    pub content: String,
    /// Annotation type (note, highlight, comment).
    #[serde(default)]
    pub annotation_type: AnnotationType,
    /// Additional metadata (position, selection range, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl CreateAnnotation {
    /// Converts to database model.
    pub fn into_model(self, file_id: Uuid, account_id: Uuid) -> NewWorkspaceFileAnnotation {
        NewWorkspaceFileAnnotation {
            file_id,
            account_id,
            content: self.content,
            annotation_type: Some(self.annotation_type),
            metadata: self.metadata,
        }
    }
}

/// Request to update an annotation.
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAnnotation {
    /// Updated content.
    #[validate(length(min = 1, max = 10000))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Updated annotation type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotation_type: Option<AnnotationType>,
    /// Updated metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl UpdateAnnotation {
    pub fn into_model(self) -> UpdateWorkspaceFileAnnotation {
        UpdateWorkspaceFileAnnotation {
            content: self.content,
            annotation_type: self.annotation_type,
            metadata: self.metadata,
            deleted_at: None,
        }
    }
}
