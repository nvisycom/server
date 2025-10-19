use std::collections::HashMap;
use std::path::Path;

use nvisy_core::fs::SupportedFormat;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::Result;

/// Comprehensive metadata for objects in the Nvisy system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMetadata {
    /// Original filename when uploaded.
    #[serde(rename = "original-filename")]
    pub original_filename: String,
    /// Timestamp when the file was uploaded.
    #[serde(rename = "uploaded-at-ts")]
    pub uploaded_at: OffsetDateTime,
    /// File UUID for tracking.
    #[serde(rename = "file-uuid")]
    pub file_uuid: Uuid,
    /// File size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Content type/MIME type.
    #[serde(rename = "content-type", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<SupportedFormat>,
    /// ETag from storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Additional custom metadata.
    #[serde(flatten)]
    pub custom: HashMap<String, String>,
}

impl ObjectMetadata {
    /// Creates new ObjectMetadata with required fields.
    pub fn new(original_filename: impl Into<String>, file_uuid: Uuid) -> Self {
        Self {
            original_filename: original_filename.into(),
            uploaded_at: OffsetDateTime::now_utc(),
            file_uuid,
            size: None,
            content_type: None,
            etag: None,
            custom: HashMap::new(),
        }
    }

    /// Sets the upload timestamp.
    pub fn with_uploaded_at(mut self, timestamp: OffsetDateTime) -> Self {
        self.uploaded_at = timestamp;
        self
    }

    /// Sets the file size in bytes.
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Sets the content type.
    pub fn with_content_type(mut self, content_type: SupportedFormat) -> Self {
        self.content_type = Some(content_type);
        self
    }

    /// Sets the ETag.
    pub fn with_etag(mut self, etag: impl Into<String>) -> Self {
        self.etag = Some(etag.into());
        self
    }

    /// Adds a custom metadata field.
    pub fn with_custom_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }

    /// Adds multiple custom metadata fields.
    pub fn with_custom_fields(mut self, fields: HashMap<String, String>) -> Self {
        self.custom.extend(fields);
        self
    }

    /// Converts to a flat HashMap for MinIO metadata.
    pub fn to_hashmap(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        metadata.insert(
            "original-filename".to_string(),
            self.original_filename.clone(),
        );
        metadata.insert(
            "uploaded-at-ts".to_string(),
            self.uploaded_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| "unknown".to_string()),
        );
        metadata.insert("file-uuid".to_string(), self.file_uuid.to_string());

        if let Some(size) = self.size {
            metadata.insert("size".to_string(), size.to_string());
        }

        if let Some(content_type) = &self.content_type {
            metadata.insert("content-type".to_string(), content_type.to_string());
        }

        if let Some(etag) = &self.etag {
            metadata.insert("etag".to_string(), etag.clone());
        }

        metadata.extend(self.custom.clone());
        metadata
    }

    /// Creates ObjectMetadata from a HashMap.
    pub fn from_hashmap(metadata: HashMap<String, String>) -> Result<Self> {
        let original_filename = metadata
            .get("original-filename")
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing 'original-filename' metadata".to_string())
            })?
            .clone();

        let uploaded_at = metadata
            .get("uploaded-at-ts")
            .and_then(|s| {
                OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
            })
            .unwrap_or_else(OffsetDateTime::now_utc);

        let file_uuid = metadata
            .get("file-uuid")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::Error::InvalidRequest("Missing or invalid 'file-uuid' metadata".to_string())
            })?;

        let size = metadata.get("size").and_then(|s| s.parse().ok());

        let content_type = metadata
            .get("content-type")
            .and_then(|s| SupportedFormat::from_extension(s.as_str()));

        let etag = metadata.get("etag").cloned();

        let mut custom = metadata;
        // Remove standard metadata from custom map
        custom.remove("original-filename");
        custom.remove("uploaded-at-ts");
        custom.remove("file-uuid");
        custom.remove("size");
        custom.remove("content-type");
        custom.remove("etag");

        Ok(Self {
            original_filename,
            uploaded_at,
            file_uuid,
            size,
            content_type,
            etag,
            custom,
        })
    }

    /// Returns the file extension if available from the original filename.
    pub fn file_extension(&self) -> Option<&str> {
        Path::new(&self.original_filename)
            .extension()
            .and_then(|ext| ext.to_str())
    }

    /// Attempts to determine the format from the original filename.
    pub fn format_from_filename(&self) -> Option<SupportedFormat> {
        self.file_extension()
            .and_then(SupportedFormat::from_extension)
    }

    /// Returns whether this appears to be a document based on format.
    pub fn is_document(&self) -> bool {
        self.content_type.as_ref().map_or_else(
            || self.format_from_filename().is_some_and(|f| f.is_document()),
            |_format| {
                // Check if format is a document type
                self.format_from_filename().is_some_and(|f| f.is_document())
            },
        )
    }

    /// Returns whether this appears to be an image based on format.
    pub fn is_image(&self) -> bool {
        self.content_type.as_ref().map_or_else(
            || self.format_from_filename().is_some_and(|f| f.is_image()),
            |_format| {
                // Check if format is an image type
                self.format_from_filename().is_some_and(|f| f.is_image())
            },
        )
    }
}
