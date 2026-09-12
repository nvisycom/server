//! Join model pinning a detection to the exact policy versions its analysis ran.
//!
//! A policy's definition is versioned; recording the resolved versions here makes
//! a detection reproducible against the config that produced it, independent of
//! later policy edits or soft-deletes.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_detection_policy_versions;

/// A detection → policy-version pin row.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Insertable)]
#[diesel(table_name = workspace_detection_policy_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DetectionPolicyVersion {
    /// Detection whose analysis pinned this version.
    pub detection_id: Uuid,
    /// Policy version the analysis consumed.
    pub policy_version_id: Uuid,
    /// Workspace both belong to.
    pub workspace_id: Uuid,
}
