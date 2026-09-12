//! Workspace policy version model: an immutable snapshot of a policy's definition.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_policy_versions;

/// An immutable version of a policy's definition.
///
/// Editing a policy's definition inserts a new version and repoints the policy's
/// `current_version_id`; a version is never updated or deleted, so a detection
/// that pinned it can always reproduce the exact definition it ran. The
/// `definition` holds the engine's `PolicyDefinition`, stored XChaCha20-Poly1305
/// encrypted with the workspace-derived key.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_policy_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspacePolicyVersion {
    /// Unique version identifier.
    pub id: Uuid,
    /// Logical policy this is a version of.
    pub policy_id: Uuid,
    /// Workspace this version belongs to.
    pub workspace_id: Uuid,
    /// Account that authored this version.
    pub account_id: Uuid,
    /// Monotonic per-policy version number (1..N).
    pub version_number: i32,
    /// Encrypted policy body (the engine's `PolicyDefinition` as JSON).
    pub definition: Vec<u8>,
    /// Definition-scoped metadata that versions with the content.
    pub metadata: JsonValue,
    /// Timestamp when the version was created.
    pub created_at: Timestamp,
}

/// Data for inserting a new policy version.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_policy_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspacePolicyVersion {
    /// Logical policy this is a version of.
    pub policy_id: Uuid,
    /// Workspace this version belongs to.
    pub workspace_id: Uuid,
    /// Account authoring this version.
    pub account_id: Uuid,
    /// Monotonic per-policy version number (1..N).
    pub version_number: i32,
    /// Encrypted policy body (the engine's `PolicyDefinition` as JSON).
    pub definition: Vec<u8>,
    /// Definition-scoped metadata that versions with the content.
    pub metadata: Option<JsonValue>,
}
