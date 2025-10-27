//! Object key type for NATS object storage.

use std::marker::PhantomData;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::object_key_data::ObjectKeyData;
use crate::{Error, Result};

/// Helper function to parse a UUID from a string with context
fn parse_uuid(uuid_str: &str, context: &str) -> Result<Uuid> {
    uuid_str.parse::<Uuid>().map_err(|e| {
        Error::operation(
            "parse_key",
            format!("Invalid {} UUID '{}': {}", context, uuid_str, e),
        )
    })
}

/// Trait for stage types that can be used in ObjectKey
pub trait DocumentLabel: Clone + Copy + Send + Sync + Sized {
    /// Get the bucket name for the label.
    fn bucket_name() -> &'static str;

    /// Get the description for the label.
    fn description() -> &'static str;

    /// Get the max age for the label.
    fn max_age() -> Duration;
}

/// Input stage marker
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
pub struct InputFiles;

impl DocumentLabel for InputFiles {
    fn bucket_name() -> &'static str {
        "input"
    }

    fn description() -> &'static str {
        "Input document files"
    }

    fn max_age() -> Duration {
        Duration::from_secs(30 * 24 * 60 * 60) // 30 days
    }
}

/// Intermediate stage marker
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
pub struct IntermediateFiles;

impl DocumentLabel for IntermediateFiles {
    fn bucket_name() -> &'static str {
        "intermediate"
    }

    fn description() -> &'static str {
        "Intermediate processing files"
    }

    fn max_age() -> Duration {
        Duration::from_secs(7 * 24 * 60 * 60) // 7 days
    }
}

/// Output stage marker
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
pub struct OutputFiles;

impl DocumentLabel for OutputFiles {
    fn bucket_name() -> &'static str {
        "output"
    }

    fn description() -> &'static str {
        "Output document files"
    }

    fn max_age() -> Duration {
        Duration::from_secs(90 * 24 * 60 * 60) // 90 days
    }
}

/// A validated string wrapper representing an object key in NATS object storage.
///
/// The key follows the format: `{project_uuid}/{document_uuid}/{file_uuid}`
///
/// # Type Parameters
/// * `S` - The stage type that implements `DocumentLabel` (used for bucket selection)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectKey<S: DocumentLabel> {
    /// The actual key string used in object storage
    key: String,
    _phantom: PhantomData<S>,
}

impl<S: DocumentLabel> ObjectKey<S> {
    /// Creates a new ObjectKey from a string without validation
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            _phantom: PhantomData,
        }
    }

    /// Constructs an ObjectKey from ObjectKeyData
    pub fn from_data(data: ObjectKeyData) -> Result<Self> {
        let key = format!(
            "{}/{}/{}",
            data.project_uuid(),
            data.document_uuid(),
            data.file_uuid()
        );
        Ok(Self {
            key,
            _phantom: PhantomData,
        })
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
        if parts.len() != 3 {
            return Err(Error::operation(
                "parse_key",
                format!(
                    "Invalid key format '{}': expected 3 parts separated by '/' (project/document/file_uuid)",
                    self.key
                ),
            ));
        }

        let project_uuid = parse_uuid(parts[0], "project")?;
        let document_uuid = parse_uuid(parts[1], "document")?;
        let file_uuid = parse_uuid(parts[2], "file")?;

        Ok(ObjectKeyData::new(project_uuid, document_uuid, file_uuid))
    }

    /// Extracts the project UUID from the key
    pub fn project_uuid(&self) -> Result<Uuid> {
        self.parse().map(|data| data.project_uuid())
    }

    /// Extracts the document UUID from the key
    pub fn document_uuid(&self) -> Result<Uuid> {
        self.parse().map(|data| data.document_uuid())
    }

    /// Extracts the file UUID from the key
    pub fn file_uuid(&self) -> Result<Uuid> {
        self.parse().map(|data| data.file_uuid())
    }
}

impl<S: DocumentLabel> FromStr for ObjectKey<S> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = ObjectKey::new(s);
        // Validate by parsing
        key.parse()?;
        Ok(key)
    }
}

impl<S: DocumentLabel> std::fmt::Display for ObjectKey<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl<S: DocumentLabel> AsRef<str> for ObjectKey<S> {
    fn as_ref(&self) -> &str {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_markers() {
        assert_eq!(InputFiles::bucket_name(), "input");
        assert_eq!(IntermediateFiles::bucket_name(), "intermediate");
        assert_eq!(OutputFiles::bucket_name(), "output");
    }

    #[test]
    fn test_stage_descriptions_and_max_age() {
        // Test descriptions
        assert_eq!(InputFiles::description(), "Input document files");
        assert_eq!(
            IntermediateFiles::description(),
            "Intermediate processing files"
        );
        assert_eq!(OutputFiles::description(), "Output document files");

        // Test max ages
        assert_eq!(
            InputFiles::max_age(),
            Duration::from_secs(30 * 24 * 60 * 60)
        ); // 30 days
        assert_eq!(
            IntermediateFiles::max_age(),
            Duration::from_secs(7 * 24 * 60 * 60)
        ); // 7 days
        assert_eq!(
            OutputFiles::max_age(),
            Duration::from_secs(90 * 24 * 60 * 60)
        ); // 90 days
    }

    #[test]
    fn test_object_key_from_data() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();

        let file_uuid = Uuid::new_v4();

        let data = ObjectKeyData::new(project_uuid, document_uuid, file_uuid);

        let key = ObjectKey::<IntermediateFiles>::from_data(data.clone()).unwrap();
        let expected = format!("{}/{}/{}", project_uuid, document_uuid, file_uuid);

        assert_eq!(key.as_str(), expected);
        assert_eq!(key.to_string(), expected);
    }

    #[test]
    fn test_object_key_accessor_methods() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let key_str = format!("{}/{}/{}", project_uuid, document_uuid, file_uuid);

        let key: ObjectKey<OutputFiles> = ObjectKey::new(key_str);

        assert_eq!(key.project_uuid().unwrap(), project_uuid);
        assert_eq!(key.document_uuid().unwrap(), document_uuid);
        assert_eq!(key.file_uuid().unwrap(), file_uuid);
    }

    #[test]
    fn test_object_key_from_str() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let key_str = format!("{}/{}/{}", project_uuid, document_uuid, file_uuid);

        let key: ObjectKey<InputFiles> = ObjectKey::from_str(&key_str).unwrap();
        assert_eq!(key.as_str(), key_str);

        // Test invalid key
        let invalid_key = "invalid/key/format";
        assert!(ObjectKey::<InputFiles>::from_str(invalid_key).is_err());
    }

    #[test]
    fn test_derive_more_display_and_as_ref() {
        let project_uuid = Uuid::new_v4();
        let document_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let key_str = format!("{}/{}/{}", project_uuid, document_uuid, file_uuid);
        let key: ObjectKey<InputFiles> = ObjectKey::new(key_str.clone());

        // Test Display derive
        assert_eq!(format!("{}", key), key_str);

        // Test AsRef derive
        let as_ref: &str = key.as_ref();
        assert_eq!(as_ref, key_str);
    }

    #[test]
    fn test_parse_uuid_helper() {
        let valid_uuid = Uuid::new_v4();
        let valid_uuid_str = valid_uuid.to_string();

        assert_eq!(parse_uuid(&valid_uuid_str, "test").unwrap(), valid_uuid);

        let invalid_uuid_str = "invalid-uuid";
        assert!(parse_uuid(invalid_uuid_str, "test").is_err());
    }
}
