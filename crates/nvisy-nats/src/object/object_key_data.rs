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
    /// Unique identifier for the workspace
    workspace_uuid: Uuid,
    /// Unique identifier for the file
    file_uuid: Uuid,
}

impl ObjectKeyData {
    /// Creates a new ObjectKeyData with required fields.
    pub fn new(workspace_uuid: Uuid, file_uuid: Uuid) -> Self {
        Self {
            workspace_uuid,
            file_uuid,
        }
    }

    /// Get the workspace UUID
    pub fn workspace_uuid(&self) -> Uuid {
        self.workspace_uuid
    }

    /// Get the file UUID
    pub fn file_uuid(&self) -> Uuid {
        self.file_uuid
    }

    /// Build an ObjectKey from this data with the specified stage
    pub fn build<S: DocumentLabel>(self) -> Result<ObjectKey<S>> {
        ObjectKey::from_data(self)
    }

    /// Creates a prefix for listing objects under a workspace
    pub fn create_prefix(workspace_uuid: Uuid) -> String {
        format!("{}/", workspace_uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::object_key::InputFiles;

    #[test]
    fn test_object_key_data_creation() {
        let workspace_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let data = ObjectKeyData::new(workspace_uuid, file_uuid);

        assert_eq!(data.workspace_uuid(), workspace_uuid);
        assert_eq!(data.file_uuid(), file_uuid);
    }

    #[test]
    fn test_build_with_stage() {
        let workspace_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        let data = ObjectKeyData::new(workspace_uuid, file_uuid);
        let key = data.build::<InputFiles>().unwrap();

        assert_eq!(key.workspace_uuid().unwrap(), workspace_uuid);
        assert_eq!(key.file_uuid().unwrap(), file_uuid);
    }

    #[test]
    fn test_prefix_utilities() {
        let workspace_uuid = Uuid::new_v4();

        let prefix = ObjectKeyData::create_prefix(workspace_uuid);
        assert_eq!(prefix, format!("{}/", workspace_uuid));
    }

    #[test]
    fn test_build_key_with_version() {
        let workspace_uuid = Uuid::new_v4();
        let file_uuid = Uuid::new_v4();

        // Test key building
        let data = ObjectKeyData::new(workspace_uuid, file_uuid);
        let key = data.build::<InputFiles>().unwrap();
        assert_eq!(key.as_str(), format!("{}/{}", workspace_uuid, file_uuid));
    }
}
