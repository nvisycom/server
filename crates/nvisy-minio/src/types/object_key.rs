//! Object key structures for MinIO storage.
//!
//! This module provides types for managing object keys in MinIO storage, including:
//! - `ObjectKeyData`: The underlying data structure that acts as a builder pattern
//! - `ObjectKey`: A validated string wrapper for object keys
//!
//! Object keys follow the format: `{project_uuid}/{document_uuid}/{stage}/{file_uuid}__{timestamp}__{filename}`

use std::fmt;
use std::str::FromStr;

use nvisy_core::fs::{DataSensitivity, SupportedFormat};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::types::Stage;
use crate::{Error, Result};

/// Core data structure representing an object key with all its components.
///
/// This struct acts as both a data container and builder pattern for creating object keys.
/// All fields are private to ensure data integrity and force usage through the provided methods.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectKeyData {
    /// Unique identifier for the project
    project_uuid: Uuid,
    /// Unique identifier for the document within the project
    document_uuid: Uuid,
    /// Processing stage (input, intermediate, output)
    stage: Stage,
    /// Unique identifier for the file within the document
    file_uuid: Uuid,
    /// Unix timestamp when the object was created/processed
    timestamp: i64,
    /// Original filename as uploaded by the user
    original_filename: String,
    /// Detected file format based on extension
    supported_format: Option<SupportedFormat>,
    /// Data sensitivity level for access control
    data_sensitivity: Option<DataSensitivity>,
}

impl ObjectKeyData {
    /// Creates a new ObjectKeyData with required fields.
    ///
    /// Auto-detects format and content type from filename extension.
    /// Sets timestamp to current time.
    pub fn new(
        project_uuid: Uuid,
        document_uuid: Uuid,
        stage: Stage,
        file_uuid: Uuid,
        original_filename: impl Into<String>,
    ) -> Self {
        let filename = original_filename.into();
        let format = Self::detect_format(&filename);

        Self {
            project_uuid,
            document_uuid,
            stage,
            file_uuid,
            timestamp: OffsetDateTime::now_utc().unix_timestamp(),
            original_filename: filename,
            supported_format: format,
            data_sensitivity: None,
        }
    }

    /// Gets the MIME type of the file based on its format
    pub fn content_type(&self) -> Option<&'static str> {
        self.supported_format.map(|f| f.mime_type())
    }

    /// Sets a custom timestamp
    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Sets timestamp from an OffsetDateTime
    pub fn with_timestamp_from_datetime(mut self, datetime: OffsetDateTime) -> Self {
        self.timestamp = datetime.unix_timestamp();
        self
    }

    /// Explicitly sets the file format
    pub fn with_format(mut self, format: SupportedFormat) -> Self {
        self.supported_format = Some(format);
        self
    }

    /// Sets the data sensitivity level
    pub fn with_sensitivity(mut self, sensitivity: DataSensitivity) -> Self {
        self.data_sensitivity = Some(sensitivity);
        self
    }

    /// Extracts the file extension from the filename
    pub fn file_extension(&self) -> Option<&str> {
        if !self.original_filename.contains('.') {
            return None;
        }
        self.original_filename
            .rsplit('.')
            .next()
            .filter(|ext| !ext.is_empty() && !ext.contains('/') && ext != &self.original_filename)
    }

    /// Attempts to detect file format from filename extension
    pub fn detect_format(filename: &str) -> Option<SupportedFormat> {
        filename
            .rsplit('.')
            .next()
            .and_then(SupportedFormat::from_extension)
    }

    /// Converts the timestamp to an OffsetDateTime
    pub fn datetime(&self) -> Result<OffsetDateTime> {
        OffsetDateTime::from_unix_timestamp(self.timestamp).map_err(|e| {
            Error::InvalidRequest(format!("Invalid timestamp {}: {}", self.timestamp, e))
        })
    }

    /// Validates the object key data for correctness
    pub fn validate(&self) -> Result<()> {
        if self.original_filename.is_empty() {
            return Err(Error::InvalidRequest(
                "Original filename cannot be empty".to_string(),
            ));
        }

        if self.original_filename.contains("__") {
            return Err(Error::InvalidRequest(
                "Original filename cannot contain '__' (reserved delimiter)".to_string(),
            ));
        }

        if self.original_filename.contains('/') {
            return Err(Error::InvalidRequest(
                "Original filename cannot contain '/' character".to_string(),
            ));
        }

        Ok(())
    }
}

impl TryFrom<ObjectKey> for ObjectKeyData {
    type Error = Error;

    fn try_from(value: ObjectKey) -> Result<Self, Self::Error> {
        value.parse()
    }
}

/// A validated string wrapper representing an object key in MinIO storage.
///
/// The key follows the format: `{project_uuid}/{document_uuid}/{stage}/{file_uuid}__{timestamp}__{filename}`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectKey {
    /// The actual key string used in MinIO
    key: String,
}

impl ObjectKey {
    /// Creates a new ObjectKey from a string without validation
    ///
    /// Use `from_str` for validation or `from_data` for construction from ObjectKeyData
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }

    /// Constructs an ObjectKey from validated ObjectKeyData
    pub fn from_data(data: ObjectKeyData) -> Result<Self> {
        data.validate()?;
        let key = format!(
            "{}/{}/{}/{}__{}__{}",
            data.project_uuid,
            data.document_uuid,
            data.stage,
            data.file_uuid,
            data.timestamp,
            data.original_filename
        );
        Ok(Self { key })
    }

    /// Returns the key as a string slice
    pub fn as_str(&self) -> &str {
        &self.key
    }

    /// Consumes the ObjectKey and returns the inner string
    pub fn into_string(self) -> String {
        self.key
    }

    /// Parses the key string into ObjectKeyData components
    pub fn parse(&self) -> Result<ObjectKeyData> {
        let parts: Vec<&str> = self.key.split('/').collect();
        if parts.len() != 4 {
            return Err(Error::InvalidRequest(format!(
                "Invalid key format '{}': expected 4 parts separated by '/'",
                self.key
            )));
        }

        let project_uuid = parts[0].parse::<Uuid>().map_err(|e| {
            Error::InvalidRequest(format!("Invalid project UUID '{}': {}", parts[0], e))
        })?;

        let document_uuid = parts[1].parse::<Uuid>().map_err(|e| {
            Error::InvalidRequest(format!("Invalid document UUID '{}': {}", parts[1], e))
        })?;

        let stage = parts[2]
            .parse::<Stage>()
            .map_err(|e| Error::InvalidRequest(format!("Invalid stage '{}': {}", parts[2], e)))?;

        let file_parts: Vec<&str> = parts[3].split("__").collect();
        if file_parts.len() != 3 {
            return Err(Error::InvalidRequest(format!(
                "Invalid file part '{}': expected format 'file_uuid__timestamp__original_filename'",
                parts[3]
            )));
        }

        let file_uuid = file_parts[0].parse::<Uuid>().map_err(|e| {
            Error::InvalidRequest(format!("Invalid file UUID '{}': {}", file_parts[0], e))
        })?;

        let timestamp = file_parts[1].parse::<i64>().map_err(|e| {
            Error::InvalidRequest(format!("Invalid timestamp '{}': {}", file_parts[1], e))
        })?;

        let original_filename = file_parts[2].to_string();
        let format = ObjectKeyData::detect_format(&original_filename);

        Ok(ObjectKeyData {
            project_uuid,
            document_uuid,
            stage,
            file_uuid,
            timestamp,
            original_filename,
            supported_format: format,
            data_sensitivity: None,
        })
    }

    /// Extracts the project UUID from the key
    pub fn project_uuid(&self) -> Result<Uuid> {
        self.parse().map(|data| data.project_uuid)
    }

    /// Extracts the document UUID from the key
    pub fn document_uuid(&self) -> Result<Uuid> {
        self.parse().map(|data| data.document_uuid)
    }

    /// Extracts the stage from the key
    pub fn stage(&self) -> Result<Stage> {
        self.parse().map(|data| data.stage)
    }

    /// Extracts the file UUID from the key
    pub fn file_uuid(&self) -> Result<Uuid> {
        self.parse().map(|data| data.file_uuid)
    }

    /// Extracts the timestamp from the key
    pub fn timestamp(&self) -> Result<i64> {
        self.parse().map(|data| data.timestamp)
    }

    /// Extracts the timestamp as OffsetDateTime from the key
    pub fn datetime(&self) -> Result<OffsetDateTime> {
        self.parse()?.datetime()
    }

    /// Extracts the original filename from the key
    pub fn original_filename(&self) -> Result<String> {
        self.parse().map(|data| data.original_filename)
    }

    /// Extracts the file format from the key
    pub fn format(&self) -> Result<Option<SupportedFormat>> {
        self.parse().map(|data| data.supported_format)
    }

    /// Creates a prefix for listing objects under a project or document
    pub fn create_prefix(project_uuid: Uuid, document_uuid: Option<Uuid>) -> String {
        match document_uuid {
            Some(doc_uuid) => format!("{}/{}/", project_uuid, doc_uuid),
            None => format!("{}/", project_uuid),
        }
    }

    /// Creates a prefix for listing objects in a specific stage
    pub fn create_stage_prefix(project_uuid: Uuid, document_uuid: Uuid, stage: Stage) -> String {
        format!("{}/{}/{}/", project_uuid, document_uuid, stage)
    }

    /// Validates that the key can be parsed correctly
    pub fn validate(&self) -> Result<()> {
        self.parse().and_then(|data| data.validate())
    }
}

impl From<ObjectKeyData> for ObjectKey {
    fn from(value: ObjectKeyData) -> Self {
        ObjectKey::from_data(value).expect("ObjectKeyData should be valid")
    }
}

impl FromStr for ObjectKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = ObjectKey::new(s);
        key.validate()?;
        Ok(key)
    }
}

impl fmt::Display for ObjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl AsRef<str> for ObjectKey {
    fn as_ref(&self) -> &str {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_key_data_validation() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        // Test empty filename
        let data = ObjectKeyData::new(project_uuid, document_uuid, Stage::Input, file_uuid, "");
        assert!(data.validate().is_err());

        // Test filename with double underscore
        let data = ObjectKeyData::new(
            project_uuid,
            document_uuid,
            Stage::Input,
            file_uuid,
            "file__name.pdf",
        );
        assert!(data.validate().is_err());

        // Test filename with slash
        let data = ObjectKeyData::new(
            project_uuid,
            document_uuid,
            Stage::Input,
            file_uuid,
            "folder/file.pdf",
        );
        assert!(data.validate().is_err());

        // Test valid filename
        let data = ObjectKeyData::new(
            project_uuid,
            document_uuid,
            Stage::Input,
            file_uuid,
            "valid-file_name.pdf",
        );
        assert!(data.validate().is_ok());
    }

    #[test]
    fn test_object_key_from_data() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let data = ObjectKeyData::new(
            project_uuid,
            document_uuid,
            Stage::Intermediate,
            file_uuid,
            "test.json",
        )
        .with_timestamp(1672531200); // 2023-01-01 00:00:00 UTC

        let key = ObjectKey::from_data(data.clone()).unwrap();
        let expected = format!(
            "{}/{}/intermediate/{}__1672531200__test.json",
            project_uuid, document_uuid, file_uuid
        );

        assert_eq!(key.as_str(), expected);
        assert_eq!(key.to_string(), expected);
    }

    #[test]
    fn test_object_key_accessor_methods() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let key_str = format!(
            "{}/{}/output/{}__1672531200__image.png",
            project_uuid, document_uuid, file_uuid
        );

        let key = ObjectKey::new(key_str);

        assert_eq!(key.project_uuid().unwrap(), project_uuid);
        assert_eq!(key.document_uuid().unwrap(), document_uuid);
        assert_eq!(key.stage().unwrap(), Stage::Output);
        assert_eq!(key.file_uuid().unwrap(), file_uuid);
        assert_eq!(key.timestamp().unwrap(), 1672531200);
        assert_eq!(key.original_filename().unwrap(), "image.png");
        assert_eq!(key.format().unwrap(), Some(SupportedFormat::Png));

        let datetime = key.datetime().unwrap();
        assert_eq!(datetime.unix_timestamp(), 1672531200);
    }

    #[test]
    fn test_object_key_from_str() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let key_str = format!(
            "{}/{}/input/{}__1672531200__valid.txt",
            project_uuid, document_uuid, file_uuid
        );

        let key = ObjectKey::from_str(&key_str).unwrap();
        assert_eq!(key.as_str(), key_str);

        // Test invalid key
        let invalid_key = "invalid/key/format";
        assert!(ObjectKey::from_str(invalid_key).is_err());
    }

    #[test]
    fn test_object_key_prefix_utilities() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();

        let prefix = ObjectKey::create_prefix(project_uuid, Some(document_uuid));
        assert_eq!(prefix, format!("{}/{}/", project_uuid, document_uuid));

        let prefix = ObjectKey::create_prefix(project_uuid, None);
        assert_eq!(prefix, format!("{}/", project_uuid));

        let stage_prefix =
            ObjectKey::create_stage_prefix(project_uuid, document_uuid, Stage::Input);
        assert_eq!(
            stage_prefix,
            format!("{}/{}/input/", project_uuid, document_uuid)
        );
    }

    #[test]
    fn test_file_extension_detection() {
        let data = ObjectKeyData::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Stage::Input,
            Uuid::new_v4(),
            "document.pdf",
        );

        assert_eq!(data.file_extension(), Some("pdf"));

        let data_no_ext = ObjectKeyData::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Stage::Input,
            Uuid::new_v4(),
            "noextension",
        );

        assert_eq!(data_no_ext.file_extension(), None);
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(
            ObjectKeyData::detect_format("test.pdf"),
            Some(SupportedFormat::Pdf)
        );
        assert_eq!(
            ObjectKeyData::detect_format("test.docx"),
            Some(SupportedFormat::Docx)
        );
        assert_eq!(ObjectKeyData::detect_format("test.unknown"), None);
        assert_eq!(ObjectKeyData::detect_format("test"), None);
    }

    #[test]
    fn test_datetime_conversion() {
        let data = ObjectKeyData::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Stage::Input,
            Uuid::new_v4(),
            "test.txt",
        )
        .with_timestamp(1672531200);

        let datetime = data.datetime().unwrap();
        assert_eq!(datetime.unix_timestamp(), 1672531200);

        // Test invalid timestamp
        let invalid_data = ObjectKeyData::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Stage::Input,
            Uuid::new_v4(),
            "test.txt",
        )
        .with_timestamp(i64::MAX);

        assert!(invalid_data.datetime().is_err());
    }
}
