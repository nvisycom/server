//! Document annotation response types.

use jiff::Timestamp;
use nvisy_postgres::model::DocumentAnnotation;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response type for a document annotation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// Unique annotation identifier.
    pub id: Uuid,
    /// File this annotation belongs to.
    pub file_id: Uuid,
    /// Account that created the annotation.
    pub account_id: Uuid,
    /// Annotation content.
    pub content: String,
    /// Annotation type.
    pub annotation_type: String,
    /// Additional metadata (position, selection, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// When the annotation was created.
    pub created_at: Timestamp,
    /// When the annotation was last updated.
    pub updated_at: Timestamp,
}

/// List of annotations.
pub type Annotations = Vec<Annotation>;

impl From<DocumentAnnotation> for Annotation {
    fn from(annotation: DocumentAnnotation) -> Self {
        let metadata = if annotation
            .metadata
            .as_object()
            .is_none_or(|obj| obj.is_empty())
        {
            None
        } else {
            Some(annotation.metadata)
        };

        Self {
            id: annotation.id,
            file_id: annotation.document_file_id,
            account_id: annotation.account_id,
            content: annotation.content,
            annotation_type: annotation.annotation_type,
            metadata,
            created_at: annotation.created_at.into(),
            updated_at: annotation.updated_at.into(),
        }
    }
}
