//! Object key types for NATS object storage.

use std::fmt;
use std::str::FromStr;

use base64::prelude::*;
use uuid::Uuid;

use crate::{Error, Result};

/// Trait for object storage keys.
///
/// Keys must be convertible to/from strings for storage addressing.
/// Each key type has a prefix that organizes objects by type in the bucket.
pub trait ObjectKey: fmt::Display + FromStr + Clone + Send + Sync + 'static {
    /// The prefix for this key type (e.g., "file_", "account_").
    const PREFIX: &'static str;
}

/// A validated key for file objects in NATS object storage.
///
/// The key is encoded as `file_` prefix followed by URL-safe base64 of the
/// concatenated workspace ID and object ID. This produces a key like
/// `file_ABC123...` from two UUIDs (32 bytes → base64).
///
/// The `object_id` is a UUID v7 generated at upload time, providing:
/// - Time-ordered keys for efficient storage and retrieval
/// - Guaranteed uniqueness within the workspace
/// - No collision with database-generated IDs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for FileKey {
    const PREFIX: &'static str = "file_";
}

impl FileKey {
    /// Generates a new file key with a fresh UUID v7 object ID.
    ///
    /// Uses UUID v7 which is time-ordered and contains randomness,
    /// making keys both sortable and collision-resistant.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates a file key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }

    /// Regenerates the object ID with a fresh UUID v7.
    ///
    /// This is useful when creating a new version of a file
    /// while keeping the same workspace association.
    pub fn regenerate(&mut self) {
        self.object_id = Uuid::now_v7();
    }

    /// Encodes the key payload as URL-safe base64.
    fn encode_payload(&self) -> String {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(self.workspace_id.as_bytes());
        bytes[16..].copy_from_slice(self.object_id.as_bytes());
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Decodes a key payload from URL-safe base64.
    fn decode_payload(s: &str) -> Result<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD.decode(s).map_err(|e| {
            Error::operation("parse_key", format!("Invalid base64 encoding: {}", e))
        })?;

        if bytes.len() != 32 {
            return Err(Error::operation(
                "parse_key",
                format!("Invalid key length: expected 32 bytes, got {}", bytes.len()),
            ));
        }

        let workspace_id = Uuid::from_slice(&bytes[..16])
            .map_err(|e| Error::operation("parse_key", format!("Invalid workspace UUID: {}", e)))?;

        let object_id = Uuid::from_slice(&bytes[16..])
            .map_err(|e| Error::operation("parse_key", format!("Invalid object UUID: {}", e)))?;

        Ok(Self::from_parts(workspace_id, object_id))
    }
}

impl fmt::Display for FileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.encode_payload())
    }
}

impl FromStr for FileKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let payload = s.strip_prefix(Self::PREFIX).ok_or_else(|| {
            Error::operation(
                "parse_key",
                format!("Invalid key prefix: expected '{}'", Self::PREFIX),
            )
        })?;
        Self::decode_payload(payload)
    }
}

/// A validated key for account-scoped objects in NATS object storage.
///
/// The key format is `account_{account_id}_{version}`, where `version` is a
/// content hash. Each avatar version is a distinct object, so a versioned URL
/// maps to immutable bytes and a stale version simply does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountAvatarKey {
    pub account_id: Uuid,
    pub version: String,
}

impl ObjectKey for AccountAvatarKey {
    const PREFIX: &'static str = "account_";
}

impl AccountAvatarKey {
    /// Creates a new account key for a specific avatar version.
    pub fn new(account_id: Uuid, version: impl Into<String>) -> Self {
        Self {
            account_id,
            version: version.into(),
        }
    }
}

impl fmt::Display for AccountAvatarKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}_{}", Self::PREFIX, self.account_id, self.version)
    }
}

impl FromStr for AccountAvatarKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let payload = s.strip_prefix(Self::PREFIX).ok_or_else(|| {
            Error::operation(
                "parse_key",
                format!("Invalid key prefix: expected '{}'", Self::PREFIX),
            )
        })?;
        let (id, version) = payload
            .split_once('_')
            .ok_or_else(|| Error::operation("parse_key", "Expected 'account_{id}_{version}'"))?;
        let account_id = Uuid::parse_str(id)
            .map_err(|e| Error::operation("parse_key", format!("Invalid account UUID: {}", e)))?;
        Ok(Self::new(account_id, version))
    }
}

/// A validated key for workspace-scoped objects in NATS object storage.
///
/// The key format is `workspace_{workspace_id}_{version}`, where `version` is a
/// content hash. Each avatar version is a distinct object, so a versioned URL
/// maps to immutable bytes and a stale version simply does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceAvatarKey {
    pub workspace_id: Uuid,
    pub version: String,
}

impl ObjectKey for WorkspaceAvatarKey {
    const PREFIX: &'static str = "workspace_";
}

impl WorkspaceAvatarKey {
    /// Creates a new workspace key for a specific avatar version.
    pub fn new(workspace_id: Uuid, version: impl Into<String>) -> Self {
        Self {
            workspace_id,
            version: version.into(),
        }
    }
}

impl fmt::Display for WorkspaceAvatarKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}_{}", Self::PREFIX, self.workspace_id, self.version)
    }
}

impl FromStr for WorkspaceAvatarKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let payload = s.strip_prefix(Self::PREFIX).ok_or_else(|| {
            Error::operation(
                "parse_key",
                format!("Invalid key prefix: expected '{}'", Self::PREFIX),
            )
        })?;
        let (id, version) = payload
            .split_once('_')
            .ok_or_else(|| Error::operation("parse_key", "Expected 'workspace_{id}_{version}'"))?;
        let workspace_id = Uuid::parse_str(id)
            .map_err(|e| Error::operation("parse_key", format!("Invalid workspace UUID: {}", e)))?;
        Ok(Self::new(workspace_id, version))
    }
}

/// A validated key for intermediate pipeline objects in NATS object storage.
///
/// Addresses a run's analyzed document — the engine's detection result held
/// between the detect and redact calls. Encoded as an `intermediate_` prefix
/// followed by URL-safe base64 of the concatenated workspace ID and object ID
/// (32 bytes -> base64), like [`FileKey`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntermediateKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for IntermediateKey {
    const PREFIX: &'static str = "intermediate_";
}

impl IntermediateKey {
    /// Generates a new intermediate key with a fresh UUID v7 object ID.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates an intermediate key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }

    /// Encodes the key payload as URL-safe base64.
    fn encode_payload(&self) -> String {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(self.workspace_id.as_bytes());
        bytes[16..].copy_from_slice(self.object_id.as_bytes());
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Decodes a key payload from URL-safe base64.
    fn decode_payload(s: &str) -> Result<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD.decode(s).map_err(|e| {
            Error::operation("parse_key", format!("Invalid base64 encoding: {}", e))
        })?;

        if bytes.len() != 32 {
            return Err(Error::operation(
                "parse_key",
                format!("Invalid key length: expected 32 bytes, got {}", bytes.len()),
            ));
        }

        let workspace_id = Uuid::from_slice(&bytes[..16])
            .map_err(|e| Error::operation("parse_key", format!("Invalid workspace UUID: {}", e)))?;

        let object_id = Uuid::from_slice(&bytes[16..])
            .map_err(|e| Error::operation("parse_key", format!("Invalid object UUID: {}", e)))?;

        Ok(Self::from_parts(workspace_id, object_id))
    }
}

impl fmt::Display for IntermediateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.encode_payload())
    }
}

impl FromStr for IntermediateKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let payload = s.strip_prefix(Self::PREFIX).ok_or_else(|| {
            Error::operation(
                "parse_key",
                format!("Invalid key prefix: expected '{}'", Self::PREFIX),
            )
        })?;
        Self::decode_payload(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod file_key {
        use super::*;

        #[test]
        fn test_prefix() {
            assert_eq!(FileKey::PREFIX, "file_");
        }

        #[test]
        fn test_generate() {
            let workspace_id = Uuid::new_v4();
            let key = FileKey::generate(workspace_id);

            assert_eq!(key.workspace_id, workspace_id);
            assert_eq!(key.object_id.get_version_num(), 7);
        }

        #[test]
        fn test_from_parts() {
            let workspace_id = Uuid::new_v4();
            let object_id = Uuid::now_v7();
            let key = FileKey::from_parts(workspace_id, object_id);

            assert_eq!(key.workspace_id, workspace_id);
            assert_eq!(key.object_id, object_id);
        }

        #[test]
        fn test_display_has_prefix() {
            let workspace_id = Uuid::new_v4();
            let key = FileKey::generate(workspace_id);
            let encoded = key.to_string();

            assert!(encoded.starts_with("file_"));
            // prefix (5) + base64 (43) = 48
            assert_eq!(encoded.len(), 48);
        }

        #[test]
        fn test_roundtrip() {
            let workspace_id = Uuid::new_v4();
            let object_id = Uuid::new_v4();

            let key = FileKey::from_parts(workspace_id, object_id);
            let encoded = key.to_string();
            let decoded: FileKey = encoded.parse().unwrap();

            assert_eq!(decoded.workspace_id, workspace_id);
            assert_eq!(decoded.object_id, object_id);
            assert_eq!(key, decoded);
        }

        #[test]
        fn test_from_str_invalid_prefix() {
            assert!(FileKey::from_str("account_abc").is_err());
            assert!(FileKey::from_str("abc").is_err());
        }
    }

    mod account_key {
        use super::*;

        #[test]
        fn test_prefix() {
            assert_eq!(AccountAvatarKey::PREFIX, "account_");
        }

        #[test]
        fn test_new() {
            let account_id = Uuid::new_v4();
            let key = AccountAvatarKey::new(account_id, "abc123");
            assert_eq!(key.account_id, account_id);
            assert_eq!(key.version, "abc123");
        }

        #[test]
        fn test_display_has_prefix() {
            let account_id = Uuid::new_v4();
            let key = AccountAvatarKey::new(account_id, "abc123");
            let encoded = key.to_string();

            assert!(encoded.starts_with("account_"));
            assert_eq!(encoded, format!("account_{account_id}_abc123"));
        }

        #[test]
        fn test_roundtrip() {
            let account_id = Uuid::new_v4();
            let key = AccountAvatarKey::new(account_id, "abc123");
            let encoded = key.to_string();
            let decoded: AccountAvatarKey = encoded.parse().unwrap();
            assert_eq!(decoded.account_id, account_id);
            assert_eq!(decoded.version, "abc123");
        }

        #[test]
        fn test_from_str_invalid_prefix() {
            assert!(AccountAvatarKey::from_str("file_abc").is_err());
            assert!(AccountAvatarKey::from_str("abc").is_err());
        }

        #[test]
        fn test_from_str_missing_version() {
            assert!(AccountAvatarKey::from_str("account_not-a-uuid").is_err());
        }
    }
}
