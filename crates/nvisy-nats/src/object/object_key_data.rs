//! Object key data structure for building object keys.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::object_key::{DocumentLabel, ObjectKey};
use crate::Result;

/// Core data structure representing an object key with all its components.
///
/// This struct acts as a builder pattern for creating object keys.
/// The stage type is used for bucket selection and type safety, not stored in the key path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
pub struct ObjectKeyData {
    /// Unique identifier for the project
    project_uuid: Uuid,
    /// Unique identifier for the file
    file_uuid: Uuid,
}

impl ObjectKeyData {
    /// Creates a new ObjectKeyData with required fields.
    pub fn new(project_uuid: Uuid, file_uuid: Uuid) -> Self {
        Self {
            project_uuid,
            file_uuid,
        }
    }

    /// Get the project UUID
    pub fn project_uuid(&self) -> Uuid {
        self.project_uuid
    }

    /// Get the file UUID
    pub fn file_uuid(&self) -> Uuid {
        self.file_uuid
    }

    /// Build an ObjectKey from this data with the specified stage
    pub fn build<S: DocumentLabel>(self) -> Result<ObjectKey<S>> {
        ObjectKey::from_data(self)
    }

    /// Creates a prefix for listing objects under a project
    pub fn create_prefix(project_uuid: Uuid) -> String {
        format!("{}/", project_uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::object_key::InputFiles;

    #[test]
    fn test_object_key_data_creation() {
        let project_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let data = ObjectKeyData::new(project_uuid, file_uuid);

        assert_eq!(data.project_uuid(), project_uuid);
        assert_eq!(data.file_uuid(), file_uuid);
    }

    #[test]
    fn test_build_with_stage() {
        let project_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let data = ObjectKeyData::new(project_uuid, file_uuid);
        let key = data.build::<InputFiles>().unwrap();

        assert_eq!(key.project_uuid().unwrap(), project_uuid);
        assert_eq!(key.file_uuid().unwrap(), file_uuid);
    }

    #[test]
    fn test_prefix_utilities() {
        let project_uuid = Uuid::new_v4();

        let prefix = ObjectKeyData::create_prefix(project_uuid);
        assert_eq!(prefix, format!("{}/", project_uuid));
    }

    #[test]
    fn test_build_key_with_version() {
        let project_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        // Test key building
        let data = ObjectKeyData::new(project_uuid, file_uuid);
        let key = data.build::<InputFiles>().unwrap();
        assert_eq!(key.as_str(), format!("{}/{}", project_uuid, file_uuid));
    }
}
