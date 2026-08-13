//! Structured metadata for the `workspace_pipelines.metadata` JSONB column.
//!
//! A column the server itself populates has a known shape, so it is typed rather
//! than left as free-form JSON. Read with `Json::or_default` so an absent or
//! older blob yields an empty value rather than failing the read.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Retention, RetentionScope};

/// Structured metadata for a pipeline.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct PipelineMetadata {
    /// Per-pipeline retention override; when absent, the workspace baseline
    /// applies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<RetentionOverride>,
    /// Free-form labels attached to the pipeline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// A pipeline's optional per-scope override of the workspace retention. A `None`
/// field inherits the workspace value for that scope.
///
/// Only scopes a pipeline actually produces are overridable — original documents
/// are ingested, not produced by a pipeline, so they have no per-pipeline
/// override and always follow the workspace baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct RetentionOverride {
    /// Overrides redacted-document retention when set.
    pub redacted_documents: Option<Retention>,
    /// Overrides audit-blob retention when set.
    pub audit_logs: Option<Retention>,
}

impl RetentionOverride {
    /// The override for `scope`, if any. Original documents have no per-pipeline
    /// override.
    #[must_use]
    pub fn get(&self, scope: RetentionScope) -> Option<Retention> {
        match scope {
            RetentionScope::OriginalDocuments => None,
            RetentionScope::RedactedDocuments => self.redacted_documents,
            RetentionScope::AuditLogs => self.audit_logs,
        }
    }
}
