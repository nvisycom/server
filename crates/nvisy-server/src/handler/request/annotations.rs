//! Annotation request types.

use nvisy_postgres::model::{NewDocumentAnnotation, UpdateDocumentAnnotation};
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

    /// Annotation type (note, highlight, comment, etc.).
    #[validate(length(min = 1, max = 50))]
    #[serde(default = "default_annotation_type")]
    pub annotation_type: String,

    /// Additional metadata (position, selection range, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

fn default_annotation_type() -> String {
    "note".to_string()
}

impl CreateAnnotation {
    /// Converts to database model.
    pub fn into_model(self, file_id: Uuid, account_id: Uuid) -> NewDocumentAnnotation {
        NewDocumentAnnotation {
            document_file_id: file_id,
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
    #[validate(length(min = 1, max = 50))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotation_type: Option<String>,

    /// Updated metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl UpdateAnnotation {
    /// Converts to database model.
    pub fn into_model(self) -> UpdateDocumentAnnotation {
        UpdateDocumentAnnotation {
            content: self.content,
            annotation_type: self.annotation_type,
            metadata: self.metadata,
        }
    }
}
