use std::collections::HashMap;

use nvisy_core::fs::{DataSensitivity, SupportedFormat};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Result, Stage};

/// Standardized object tags for the Nvisy system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectTags {
    /// Processing stage.
    pub stage: Stage,
    /// File format.
    pub format: SupportedFormat,
    /// Data sensitivity level.
    pub sensitivity: DataSensitivity,
    /// Project UUID.
    pub project: Uuid,
    /// Document UUID.
    pub document: Uuid,
    /// File UUID.
    pub file: Uuid,
    /// Additional custom tags.
    #[serde(flatten)]
    pub custom: HashMap<String, String>,
}

impl ObjectTags {
    /// Creates new ObjectTags with required fields.
    pub fn new(
        stage: Stage,
        format: SupportedFormat,
        sensitivity: DataSensitivity,
        project: Uuid,
        document: Uuid,
        file: Uuid,
    ) -> Self {
        Self {
            stage,
            format,
            sensitivity,
            project,
            document,
            file,
            custom: HashMap::new(),
        }
    }

    /// Adds a custom tag.
    pub fn with_custom_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }

    /// Adds multiple custom tags.
    pub fn with_custom_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.custom.extend(tags);
        self
    }

    /// Converts to a flat HashMap for MinIO tagging.
    pub fn to_hashmap(&self) -> HashMap<String, String> {
        let mut tags = HashMap::new();
        tags.insert("stage".to_string(), self.stage.to_string());
        tags.insert("format".to_string(), self.format.to_string());
        tags.insert("sensitivity".to_string(), self.sensitivity.to_string());
        tags.insert("project".to_string(), self.project.to_string());
        tags.insert("document".to_string(), self.document.to_string());
        tags.insert("file".to_string(), self.file.to_string());
        tags.extend(self.custom.clone());
        tags
    }

    /// Creates ObjectTags from a HashMap.
    pub fn from_hashmap(tags: HashMap<String, String>) -> Result<Self> {
        let stage = tags
            .get("stage")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing or invalid 'stage' tag".to_string())
            })?;

        let format = tags
            .get("format")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing or invalid 'format' tag".to_string())
            })?;

        let sensitivity = tags
            .get("sensitivity")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing or invalid 'sensitivity' tag".to_string())
            })?;

        let project = tags
            .get("project")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing or invalid 'project' tag".to_string())
            })?;

        let document = tags
            .get("document")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing or invalid 'document' tag".to_string())
            })?;

        let file = tags
            .get("file")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing or invalid 'file' tag".to_string())
            })?;

        let mut custom = tags;
        // Remove standard tags from custom map
        custom.remove("stage");
        custom.remove("format");
        custom.remove("sensitivity");
        custom.remove("project");
        custom.remove("document");
        custom.remove("file");

        Ok(Self {
            stage,
            format,
            sensitivity,
            project,
            document,
            file,
            custom,
        })
    }
}
