//! Project pipeline model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::project_pipelines;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt};

/// Project pipeline model representing a processing pipeline configuration.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectPipeline {
    /// Unique pipeline identifier.
    pub id: Uuid,
    /// Reference to the project this pipeline belongs to.
    pub project_id: Uuid,
    /// Human-readable pipeline name for display.
    pub display_name: String,
    /// Optional pipeline description.
    pub description: Option<String>,
    /// Type of processing pipeline.
    pub pipeline_type: String,
    /// Whether the pipeline is active.
    pub is_active: bool,
    /// Whether this is the default pipeline for this type in the project.
    pub is_default: bool,
    /// Pipeline configuration (JSON).
    pub configuration: serde_json::Value,
    /// Pipeline settings (JSON).
    pub settings: serde_json::Value,
    /// Reference to the account that created this pipeline.
    pub created_by: Uuid,
    /// Timestamp when the pipeline was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the pipeline was soft-deleted.
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new project pipeline.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = project_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectPipeline {
    /// Project ID.
    pub project_id: Uuid,
    /// Display name.
    pub display_name: String,
    /// Description.
    pub description: Option<String>,
    /// Pipeline type.
    pub pipeline_type: Option<String>,
    /// Is active flag.
    pub is_active: Option<bool>,
    /// Is default flag.
    pub is_default: Option<bool>,
    /// Configuration.
    pub configuration: Option<serde_json::Value>,
    /// Settings.
    pub settings: Option<serde_json::Value>,
    /// Created by.
    pub created_by: Uuid,
}

/// Data for updating a project pipeline.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectPipeline {
    /// Display name.
    pub display_name: Option<String>,
    /// Description.
    pub description: Option<Option<String>>,
    /// Pipeline type.
    pub pipeline_type: Option<String>,
    /// Is active flag.
    pub is_active: Option<bool>,
    /// Is default flag.
    pub is_default: Option<bool>,
    /// Configuration.
    pub configuration: Option<serde_json::Value>,
    /// Settings.
    pub settings: Option<serde_json::Value>,
}

impl ProjectPipeline {
    /// Returns whether the pipeline was created recently.
    pub fn is_recently_created(&self) -> bool {
        self.was_created_within(time::Duration::hours(24))
    }

    /// Returns whether the pipeline is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the pipeline is active and not deleted.
    pub fn is_available(&self) -> bool {
        self.is_active && !self.is_deleted()
    }

    /// Returns whether the pipeline has custom configuration.
    pub fn has_configuration(&self) -> bool {
        !self
            .configuration
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the pipeline has custom settings.
    pub fn has_settings(&self) -> bool {
        !self.settings.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the pipeline has a description.
    pub fn has_description(&self) -> bool {
        self.description
            .as_ref()
            .is_some_and(|desc| !desc.trim().is_empty())
    }

    /// Returns the pipeline type in title case.
    pub fn pipeline_type_title(&self) -> String {
        self.pipeline_type
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Returns whether the pipeline is of a specific type.
    pub fn is_type(&self, pipeline_type: &str) -> bool {
        self.pipeline_type.eq_ignore_ascii_case(pipeline_type)
    }

    /// Returns whether the pipeline is a document processing pipeline.
    pub fn is_document_pipeline(&self) -> bool {
        self.is_type("document")
    }

    /// Returns whether the pipeline is an image processing pipeline.
    pub fn is_image_pipeline(&self) -> bool {
        self.is_type("image")
    }

    /// Returns whether the pipeline is a custom pipeline.
    pub fn is_custom_pipeline(&self) -> bool {
        !matches!(
            self.pipeline_type.as_str(),
            "document" | "image" | "text" | "audio" | "video"
        )
    }

    /// Returns the pipeline status description.
    pub fn status_description(&self) -> &'static str {
        match (self.is_active, self.is_deleted()) {
            (true, false) => "Active",
            (false, false) => "Inactive",
            (_, true) => "Deleted",
        }
    }

    /// Returns a configuration value by key.
    pub fn get_config_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.configuration.get(key)
    }

    /// Returns a settings value by key.
    pub fn get_setting_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.settings.get(key)
    }

    /// Returns whether this pipeline can be used for processing.
    pub fn can_process(&self) -> bool {
        self.is_available() && self.has_configuration()
    }
}

impl HasCreatedAt for ProjectPipeline {
    fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }
}

impl HasUpdatedAt for ProjectPipeline {
    fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

impl HasDeletedAt for ProjectPipeline {
    fn deleted_at(&self) -> Option<OffsetDateTime> {
        self.deleted_at
    }
}
