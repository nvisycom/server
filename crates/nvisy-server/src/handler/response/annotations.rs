//! Document annotation response types.

use jiff::Timestamp;
use nvisy_postgres::model::DocumentAnnotation;
use nvisy_postgres::types::AnnotationType;
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
    pub annotation_type: AnnotationType,
    /// When the annotation was created.
    pub created_at: Timestamp,
    /// When the annotation was last updated.
    pub updated_at: Timestamp,
}

/// List of annotations.
pub type Annotations = Vec<Annotation>;

impl Annotation {
    /// Creates an Annotation response from a database model.
    pub fn from_model(annotation: DocumentAnnotation) -> Self {
        Self {
            id: annotation.id,
            file_id: annotation.document_file_id,
            account_id: annotation.account_id,
            content: annotation.content,
            annotation_type: annotation.annotation_type,
            created_at: annotation.created_at.into(),
            updated_at: annotation.updated_at.into(),
        }
    }

    /// Creates a list of Annotation responses from database models.
    pub fn from_models(models: Vec<DocumentAnnotation>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }
}
