//! Structured metadata for the `workspaces.metadata` JSONB column.
//!
//! A column the server itself populates has a known shape, so it is typed rather
//! than left as free-form JSON. Read with `Json::or_default` so an absent or
//! older blob yields an empty value rather than failing the read.

use serde::{Deserialize, Serialize};

/// Structured metadata for a workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct WorkspaceMetadata {
    /// Free-form labels attached to the workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}
