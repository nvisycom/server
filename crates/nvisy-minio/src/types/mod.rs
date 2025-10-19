//! Types and data structures for MinIO operations.
//!
//! This module provides comprehensive types for working with MinIO objects,
//! metadata, policies, and other storage-related structures specific to the
//! Nvisy document processing system.

use bytes::Bytes;
use futures::stream::Stream;
use nvisy_core::fs::{DataSensitivity, SupportedFormat};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

mod bucket_info;
mod download_context;
mod object_info;
mod object_key;
mod object_metadata;
mod object_stage;
mod object_tags;
mod upload_context;

pub use bucket_info::BucketInfo;
pub use download_context::DownloadContext;
pub use object_info::ObjectInfo;
pub use object_key::{ObjectKey, ObjectKeyData};
pub use object_metadata::ObjectMetadata;
pub use object_stage::Stage;
pub use object_tags::ObjectTags;
pub use upload_context::UploadContext;

use crate::{Error, Result};

/// Represents an object in MinIO storage as a stream.
///
/// This struct provides a streaming interface for handling large objects
/// without loading them entirely into memory.
pub struct Object {
    /// Object key/path in storage.
    pub key: String,
    /// Object metadata.
    pub metadata: ObjectMetadata,
    /// Object tags.
    pub tags: ObjectTags,
    /// Content stream (boxed to avoid generic parameters).
    pub stream: Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send + Unpin>,
}

impl Object {
    /// Creates a new Object with the provided stream.
    pub fn new<S>(
        key: impl Into<String>,
        metadata: ObjectMetadata,
        tags: ObjectTags,
        stream: S,
    ) -> Self
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Send + Unpin + 'static,
    {
        Self {
            key: key.into(),
            metadata,
            tags,
            stream: Box::new(stream),
        }
    }

    /// Creates an Object from bytes.
    pub fn from_bytes(
        key: impl Into<String>,
        metadata: ObjectMetadata,
        tags: ObjectTags,
        data: Bytes,
    ) -> Self {
        let stream = Box::pin(futures::stream::once(async move { Ok(data) }));
        Self {
            key: key.into(),
            metadata,
            tags,
            stream: Box::new(stream),
        }
    }

    /// Gets the object size if available in metadata.
    pub fn size(&self) -> Option<u64> {
        self.metadata.size
    }

    /// Gets the content type (MIME type) as a string.
    pub fn content_type(&self) -> Option<&'static str> {
        self.metadata.content_type.map(|format| format.mime_type())
    }

    /// Gets the original filename from metadata.
    pub fn original_filename(&self) -> &str {
        &self.metadata.original_filename
    }

    /// Gets the file UUID from metadata.
    pub fn file_uuid(&self) -> Uuid {
        self.metadata.file_uuid
    }

    /// Gets the project UUID from tags.
    pub fn project_uuid(&self) -> Uuid {
        self.tags.project
    }

    /// Gets the document UUID from tags.
    pub fn document_uuid(&self) -> Uuid {
        self.tags.document
    }

    /// Gets the processing stage from tags.
    pub fn stage(&self) -> Stage {
        self.tags.stage
    }

    /// Gets the format from tags.
    pub fn format(&self) -> SupportedFormat {
        self.tags.format
    }

    /// Gets the sensitivity level from tags.
    pub fn sensitivity(&self) -> DataSensitivity {
        self.tags.sensitivity
    }
}

/// Bucket policy configuration for access control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketPolicy {
    /// Policy version (usually "2012-10-17").
    #[serde(rename = "Version")]
    pub version: String,
    /// Policy statements.
    #[serde(rename = "Statement")]
    pub statements: Vec<PolicyStatement>,
}

impl BucketPolicy {
    /// Creates a new empty bucket policy.
    pub fn new() -> Self {
        Self {
            version: "2012-10-17".to_string(),
            statements: Vec::new(),
        }
    }

    /// Adds a policy statement.
    pub fn with_statement(mut self, statement: PolicyStatement) -> Self {
        self.statements.push(statement);
        self
    }

    /// Creates a policy with basic read access for a bucket.
    pub fn with_read_access(self) -> Self {
        self.with_statement(
            PolicyStatement::new("Allow")
                .with_action("s3:GetObject")
                .with_action("s3:GetObjectVersion"),
        )
    }

    /// Creates a policy with basic write access for a bucket.
    pub fn with_write_access(self) -> Self {
        self.with_statement(
            PolicyStatement::new("Allow")
                .with_action("s3:PutObject")
                .with_action("s3:DeleteObject"),
        )
    }

    /// Sets the bucket name for resource ARNs.
    pub fn with_bucket_name(mut self, bucket_name: &str) -> Self {
        let resource = format!("arn:aws:s3:::{}/*", bucket_name);
        for statement in &mut self.statements {
            if statement.resource.is_empty() {
                statement.resource = vec![resource.clone()];
            }
        }
        self
    }

    /// Converts the policy to a JSON string.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(Error::from)
    }

    /// Creates a policy from a JSON string.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Error::from)
    }
}

impl Default for BucketPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Individual policy statement within a bucket policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyStatement {
    /// Statement effect ("Allow" or "Deny").
    #[serde(rename = "Effect")]
    pub effect: String,
    /// Actions this statement applies to.
    #[serde(rename = "Action")]
    pub action: Vec<String>,
    /// Resources this statement applies to.
    #[serde(rename = "Resource")]
    pub resource: Vec<String>,
    /// Optional principal specification.
    #[serde(rename = "Principal", skip_serializing_if = "Option::is_none")]
    pub principal: Option<serde_json::Value>,
}

impl PolicyStatement {
    /// Creates a new policy statement.
    pub fn new(effect: impl Into<String>) -> Self {
        Self {
            effect: effect.into(),
            action: Vec::new(),
            resource: Vec::new(),
            principal: None,
        }
    }

    /// Adds an action to the statement.
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action.push(action.into());
        self
    }

    /// Adds a resource to the statement.
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource.push(resource.into());
        self
    }

    /// Sets the principal for the statement.
    pub fn with_principal(mut self, principal: serde_json::Value) -> Self {
        self.principal = Some(principal);
        self
    }
}

/// Utility functions for working with object keys and paths.
pub mod key_utils {
    use super::*;

    /// Generates a standard Nvisy object key.
    ///
    /// Format: `<project_uuid>/<document_uuid>/<stage>/<file_uuid>__<ts>__<orig>`
    pub fn generate_key(
        project_uuid: Uuid,
        document_uuid: Uuid,
        stage: Stage,
        file_uuid: Uuid,
        timestamp: Option<OffsetDateTime>,
        original_filename: &str,
    ) -> String {
        let ts = timestamp
            .unwrap_or_else(OffsetDateTime::now_utc)
            .unix_timestamp();

        format!(
            "{}/{}/{}/{}__{}__{}",
            project_uuid, document_uuid, stage, file_uuid, ts, original_filename
        )
    }

    /// Parses a Nvisy object key into its components.
    ///
    /// Returns a tuple of (project_uuid, document_uuid, stage, file_uuid, timestamp, original_filename).
    pub fn parse_key(key: &str) -> Result<(Uuid, Uuid, Stage, Uuid, i64, String)> {
        let parts: Vec<&str> = key.split('/').collect();
        if parts.len() != 4 {
            return Err(Error::InvalidRequest(format!(
                "Invalid key format '{}': expected 4 parts separated by '/'",
                key
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

        // Parse the file part: <file_uuid>__<ts>__<orig>
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

        Ok((
            project_uuid,
            document_uuid,
            stage,
            file_uuid,
            timestamp,
            original_filename,
        ))
    }

    /// Extracts the project UUID from an object key.
    pub fn extract_project_uuid(key: &str) -> Result<Uuid> {
        let (project_uuid, _, _, _, _, _) = parse_key(key)?;
        Ok(project_uuid)
    }

    /// Extracts the document UUID from an object key.
    pub fn extract_document_uuid(key: &str) -> Result<Uuid> {
        let (_, document_uuid, _, _, _, _) = parse_key(key)?;
        Ok(document_uuid)
    }

    /// Extracts the stage from an object key.
    pub fn extract_stage(key: &str) -> Result<Stage> {
        let (_, _, stage, _, _, _) = parse_key(key)?;
        Ok(stage)
    }

    /// Extracts the file UUID from an object key.
    pub fn extract_file_uuid(key: &str) -> Result<Uuid> {
        let (_, _, _, file_uuid, _, _) = parse_key(key)?;
        Ok(file_uuid)
    }

    /// Creates a prefix for listing objects by project and document.
    pub fn create_prefix(project_uuid: Uuid, document_uuid: Option<Uuid>) -> String {
        match document_uuid {
            Some(doc_uuid) => format!("{}/{}/", project_uuid, doc_uuid),
            None => format!("{}/", project_uuid),
        }
    }

    /// Creates a prefix for listing objects by project, document, and stage.
    pub fn create_stage_prefix(project_uuid: Uuid, document_uuid: Uuid, stage: Stage) -> String {
        format!("{}/{}/{}/", project_uuid, document_uuid, stage)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_stage_enum() {
        assert_eq!(Stage::Input.to_string(), "input");
        assert_eq!(Stage::Intermediate.to_string(), "intermediate");
        assert_eq!(Stage::Output.to_string(), "output");

        assert_eq!(Stage::from_str("input").unwrap(), Stage::Input);
        assert_eq!(
            Stage::from_str("intermediate").unwrap(),
            Stage::Intermediate
        );
        assert_eq!(Stage::from_str("output").unwrap(), Stage::Output);

        assert!(Stage::Input.is_input());
        assert!(Stage::Intermediate.is_intermediate());
        assert!(Stage::Output.is_output());
    }

    #[test]
    fn test_object_tags() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let tags = ObjectTags::new(
            Stage::Input,
            SupportedFormat::Pdf,
            DataSensitivity::Medium,
            project_uuid,
            document_uuid,
            file_uuid,
        )
        .with_custom_tag("environment", "test");

        let hashmap = tags.to_hashmap();
        assert_eq!(hashmap.get("stage"), Some(&"input".to_string()));
        assert_eq!(hashmap.get("format"), Some(&"pdf".to_string()));
        assert_eq!(hashmap.get("sensitivity"), Some(&"Medium".to_string()));
        assert_eq!(hashmap.get("project"), Some(&project_uuid.to_string()));
        assert_eq!(hashmap.get("environment"), Some(&"test".to_string()));

        let parsed_tags = ObjectTags::from_hashmap(hashmap).unwrap();
        assert_eq!(parsed_tags.stage, Stage::Input);
        assert_eq!(parsed_tags.format, SupportedFormat::Pdf);
        assert_eq!(parsed_tags.sensitivity, DataSensitivity::Medium);
        assert_eq!(parsed_tags.project, project_uuid);
        assert_eq!(
            parsed_tags.custom.get("environment"),
            Some(&"test".to_string())
        );
    }

    #[test]
    fn test_object_metadata() {
        let file_uuid = Uuid::new_v4();
        let timestamp = OffsetDateTime::now_utc();

        let metadata = ObjectMetadata::new("invoice.pdf", file_uuid)
            .with_uploaded_at(timestamp)
            .with_size(1024)
            .with_content_type(SupportedFormat::Pdf)
            .with_custom_field("department", "finance");

        assert_eq!(metadata.original_filename, "invoice.pdf");
        assert_eq!(metadata.file_uuid, file_uuid);
        assert_eq!(metadata.size, Some(1024));
        assert!(metadata.content_type.is_some());
        assert_eq!(
            metadata.custom.get("department"),
            Some(&"finance".to_string())
        );

        assert_eq!(metadata.file_extension(), Some("pdf"));
        assert_eq!(metadata.format_from_filename(), Some(SupportedFormat::Pdf));

        let hashmap = metadata.to_hashmap();
        assert!(hashmap.contains_key("original-filename"));
        assert!(hashmap.contains_key("file-uuid"));
        assert!(hashmap.contains_key("department"));

        let parsed_metadata = ObjectMetadata::from_hashmap(hashmap).unwrap();
        assert_eq!(parsed_metadata.original_filename, "invoice.pdf");
        assert_eq!(parsed_metadata.file_uuid, file_uuid);
        assert_eq!(
            parsed_metadata.custom.get("department"),
            Some(&"finance".to_string())
        );
    }

    #[test]
    fn test_bucket_policy() {
        let policy = BucketPolicy::new()
            .with_statement(
                PolicyStatement::new("Allow")
                    .with_action("s3:GetObject")
                    .with_resource("arn:aws:s3:::test-bucket/*"),
            )
            .with_bucket_name("test-bucket");

        assert_eq!(policy.version, "2012-10-17");
        assert_eq!(policy.statements.len(), 1);
        assert_eq!(policy.statements[0].effect, "Allow");
        assert!(
            policy.statements[0]
                .action
                .contains(&"s3:GetObject".to_string())
        );

        let json = policy.to_json().unwrap();
        assert!(json.contains("Allow"));
        assert!(json.contains("s3:GetObject"));

        let parsed_policy = BucketPolicy::from_json(&json).unwrap();
        assert_eq!(parsed_policy.version, policy.version);
        assert_eq!(parsed_policy.statements.len(), policy.statements.len());
    }

    #[test]
    fn test_key_utils() {
        use key_utils::*;

        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();
        let timestamp = OffsetDateTime::from_unix_timestamp(1609459200).unwrap();

        // Test key generation
        let key = generate_key(
            project_uuid,
            document_uuid,
            Stage::Input,
            file_uuid,
            Some(timestamp),
            "invoice.pdf",
        );

        assert!(key.contains(&project_uuid.to_string()));
        assert!(key.contains(&document_uuid.to_string()));
        assert!(key.contains("input"));
        assert!(key.contains(&file_uuid.to_string()));
        assert!(key.contains("1609459200"));
        assert!(key.contains("invoice.pdf"));

        // Test key parsing
        let (parsed_project, parsed_doc, parsed_stage, parsed_file, parsed_ts, parsed_filename) =
            parse_key(&key).unwrap();

        assert_eq!(parsed_project, project_uuid);
        assert_eq!(parsed_doc, document_uuid);
        assert_eq!(parsed_stage, Stage::Input);
        assert_eq!(parsed_file, file_uuid);
        assert_eq!(parsed_ts, 1609459200);
        assert_eq!(parsed_filename, "invoice.pdf");

        // Test extraction functions
        assert_eq!(extract_project_uuid(&key).unwrap(), project_uuid);
        assert_eq!(extract_document_uuid(&key).unwrap(), document_uuid);
        assert_eq!(extract_stage(&key).unwrap(), Stage::Input);
        assert_eq!(extract_file_uuid(&key).unwrap(), file_uuid);

        // Test prefixes
        let prefix = create_prefix(project_uuid, Some(document_uuid));
        assert_eq!(prefix, format!("{}/{}/", project_uuid, document_uuid));

        let stage_prefix = create_stage_prefix(project_uuid, document_uuid, Stage::Input);
        assert_eq!(
            stage_prefix,
            format!("{}/{}/input/", project_uuid, document_uuid)
        );
    }

    #[test]
    fn test_invalid_key_parsing() {
        use key_utils::*;

        // Test invalid key format
        assert!(parse_key("invalid-key").is_err());
        assert!(parse_key("a/b/c").is_err()); // Too few parts
        assert!(parse_key("a/b/c/d/e").is_err()); // Too many parts

        // Test invalid UUID
        assert!(parse_key("invalid-uuid/b/input/c__123__file.pdf").is_err());

        // Test invalid stage
        assert!(
            parse_key(&format!(
                "{}/{}/invalid-stage/{}",
                Uuid::new_v4(),
                Uuid::new_v4(),
                "file__123__test.pdf"
            ))
            .is_err()
        );

        // Test invalid file format
        let key = format!(
            "{}/{}/input/invalid-file-format",
            Uuid::new_v4(),
            Uuid::new_v4()
        );
        assert!(parse_key(&key).is_err());
    }

    #[test]
    fn test_object_creation() {
        use bytes::Bytes;

        let file_uuid = Uuid::new_v4();
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();

        let metadata = ObjectMetadata::new("test.pdf", file_uuid);
        let tags = ObjectTags::new(
            Stage::Input,
            SupportedFormat::Pdf,
            DataSensitivity::Low,
            project_uuid,
            document_uuid,
            file_uuid,
        );

        let data = Bytes::from("test data");
        let object = Object::from_bytes("test-key", metadata, tags, data.clone());

        assert_eq!(object.key, "test-key");
        assert_eq!(object.original_filename(), "test.pdf");
        assert_eq!(object.file_uuid(), file_uuid);
        assert_eq!(object.project_uuid(), project_uuid);
        assert_eq!(object.document_uuid(), document_uuid);
        assert_eq!(object.stage(), Stage::Input);
        assert_eq!(object.format(), SupportedFormat::Pdf);
        assert_eq!(object.sensitivity(), DataSensitivity::Low);
    }
}
