//! Object key types for the blob store.

use std::fmt;
use std::str::FromStr;

use base64::prelude::*;
use uuid::Uuid;

use crate::bucket::Bucket;
use crate::error::{S3Error, S3Result};

/// Trait for object storage keys.
///
/// Keys must be convertible to/from strings for storage addressing. Each key
/// type has a prefix that organizes objects by type within its store, and names
/// the [`Bucket`] it belongs to — so the store derives the target bucket from
/// the key alone and a key can never be written to the wrong store.
pub trait ObjectKey: fmt::Display + FromStr + Clone + Send + Sync + 'static {
    /// The prefix for this key type (e.g. `file_`, `account_`).
    const PREFIX: &'static str;

    /// The logical store this key addresses.
    const BUCKET: Bucket;
}

/// Builds a key-parse error for `operation` from any displayable cause.
fn parse_error(message: impl std::fmt::Display) -> S3Error {
    S3Error::operation("parse_key", message)
}

/// A validated key for file objects.
///
/// The key is encoded as a `file_` prefix followed by URL-safe base64 of the
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
    const BUCKET: Bucket = Bucket::Files;
    const PREFIX: &'static str = "file_";
}

impl FileKey {
    /// Generates a new file key with a fresh UUID v7 object ID.
    ///
    /// Uses UUID v7, which is time-ordered and contains randomness, making keys
    /// both sortable and collision-resistant.
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
    /// Useful when creating a new version of a file while keeping the same
    /// workspace association.
    pub fn regenerate(&mut self) {
        self.object_id = Uuid::now_v7();
    }

    /// Encodes the key payload as URL-safe base64.
    fn encode_payload(&self) -> String {
        encode_ids(self.workspace_id, self.object_id)
    }

    /// Decodes a key payload from URL-safe base64.
    fn decode_payload(s: &str) -> S3Result<Self> {
        let (workspace_id, object_id) = decode_ids(s)?;
        Ok(Self::from_parts(workspace_id, object_id))
    }
}

impl fmt::Display for FileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.encode_payload())
    }
}

impl FromStr for FileKey {
    type Err = S3Error;

    fn from_str(s: &str) -> S3Result<Self> {
        Self::decode_payload(strip_prefix::<Self>(s)?)
    }
}

/// A validated key for a redaction audit object.
///
/// Addresses a detection's analyzed document — the engine's detection result
/// (the audit of what was found and redacted). Encoded as an `audit_` prefix
/// followed by URL-safe base64 of the concatenated workspace ID and object ID
/// (32 bytes → base64), like [`FileKey`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuditKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for AuditKey {
    const BUCKET: Bucket = Bucket::Audits;
    const PREFIX: &'static str = "audit_";
}

impl AuditKey {
    /// Generates a new audit key with a fresh UUID v7 object ID.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates an audit key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }
}

impl fmt::Display for AuditKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            Self::PREFIX,
            encode_ids(self.workspace_id, self.object_id)
        )
    }
}

impl FromStr for AuditKey {
    type Err = S3Error;

    fn from_str(s: &str) -> S3Result<Self> {
        let (workspace_id, object_id) = decode_ids(strip_prefix::<Self>(s)?)?;
        Ok(Self::from_parts(workspace_id, object_id))
    }
}

/// A validated key for an account-scoped object (an avatar).
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
    const BUCKET: Bucket = Bucket::AccountAvatars;
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
    type Err = S3Error;

    fn from_str(s: &str) -> S3Result<Self> {
        let (id, version) = split_id_version::<Self>(s)?;
        let account_id =
            Uuid::parse_str(id).map_err(|e| parse_error(format!("Invalid account UUID: {e}")))?;
        Ok(Self::new(account_id, version))
    }
}

/// A validated key for a workspace-scoped object (an avatar/logo).
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
    const BUCKET: Bucket = Bucket::WorkspaceAvatars;
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
    type Err = S3Error;

    fn from_str(s: &str) -> S3Result<Self> {
        let (id, version) = split_id_version::<Self>(s)?;
        let workspace_id =
            Uuid::parse_str(id).map_err(|e| parse_error(format!("Invalid workspace UUID: {e}")))?;
        Ok(Self::new(workspace_id, version))
    }
}

/// Strips a key type's prefix, erroring if it is absent.
fn strip_prefix<K: ObjectKey>(s: &str) -> S3Result<&str> {
    s.strip_prefix(K::PREFIX)
        .ok_or_else(|| parse_error(format!("Invalid key prefix: expected '{}'", K::PREFIX)))
}

/// Splits a prefix-stripped `{id}_{version}` payload into its two parts.
fn split_id_version<K: ObjectKey>(s: &str) -> S3Result<(&str, &str)> {
    strip_prefix::<K>(s)?
        .split_once('_')
        .ok_or_else(|| parse_error(format!("Expected '{}{{id}}_{{version}}'", K::PREFIX)))
}

/// Encodes two UUIDs (32 bytes) as URL-safe base64.
fn encode_ids(first: Uuid, second: Uuid) -> String {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(first.as_bytes());
    bytes[16..].copy_from_slice(second.as_bytes());
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

/// Decodes a URL-safe base64 payload back into two UUIDs.
fn decode_ids(s: &str) -> S3Result<(Uuid, Uuid)> {
    let bytes = BASE64_URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| parse_error(format!("Invalid base64 encoding: {e}")))?;

    if bytes.len() != 32 {
        return Err(parse_error(format!(
            "Invalid key length: expected 32 bytes, got {}",
            bytes.len()
        )));
    }

    let first = Uuid::from_slice(&bytes[..16])
        .map_err(|e| parse_error(format!("Invalid workspace UUID: {e}")))?;
    let second = Uuid::from_slice(&bytes[16..])
        .map_err(|e| parse_error(format!("Invalid object UUID: {e}")))?;
    Ok((first, second))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod file_key {
        use super::*;

        #[test]
        fn prefix_is_file() {
            assert_eq!(FileKey::PREFIX, "file_");
        }

        #[test]
        fn generate_uses_uuid_v7() {
            let workspace_id = Uuid::new_v4();
            let key = FileKey::generate(workspace_id);
            assert_eq!(key.workspace_id, workspace_id);
            assert_eq!(key.object_id.get_version_num(), 7);
        }

        #[test]
        fn display_has_prefix_and_expected_length() {
            let key = FileKey::generate(Uuid::new_v4());
            let encoded = key.to_string();
            assert!(encoded.starts_with("file_"));
            // prefix (5) + base64 of 32 bytes (43) = 48.
            assert_eq!(encoded.len(), 48);
        }

        #[test]
        fn round_trips_through_string() {
            let key = FileKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
            let decoded: FileKey = key.to_string().parse().unwrap();
            assert_eq!(key, decoded);
        }

        #[test]
        fn rejects_wrong_prefix() {
            assert!(FileKey::from_str("audit_abc").is_err());
            assert!(FileKey::from_str("abc").is_err());
        }
    }

    mod audit_key {
        use super::*;

        #[test]
        fn round_trips_and_rejects_wrong_prefix() {
            let key = AuditKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
            let decoded: AuditKey = key.to_string().parse().unwrap();
            assert_eq!(key, decoded);
            assert!(key.to_string().starts_with("audit_"));
            assert!(AuditKey::from_str("file_abc").is_err());
        }
    }

    mod avatar_keys {
        use super::*;

        #[test]
        fn account_avatar_round_trips() {
            let account_id = Uuid::new_v4();
            let key = AccountAvatarKey::new(account_id, "abc123");
            assert_eq!(key.to_string(), format!("account_{account_id}_abc123"));
            let decoded: AccountAvatarKey = key.to_string().parse().unwrap();
            assert_eq!(decoded.account_id, account_id);
            assert_eq!(decoded.version, "abc123");
        }

        #[test]
        fn workspace_avatar_round_trips() {
            let workspace_id = Uuid::new_v4();
            let key = WorkspaceAvatarKey::new(workspace_id, "v9");
            let decoded: WorkspaceAvatarKey = key.to_string().parse().unwrap();
            assert_eq!(decoded.workspace_id, workspace_id);
            assert_eq!(decoded.version, "v9");
        }

        #[test]
        fn account_avatar_rejects_bad_input() {
            assert!(AccountAvatarKey::from_str("file_abc").is_err());
            assert!(AccountAvatarKey::from_str("account_not-a-uuid").is_err());
        }
    }
}
