//! Typed workspace settings: the structured view of the `workspaces.settings`
//! JSONB column.
//!
//! Every configurable workspace preference lives here rather than as its own
//! column, so adding a setting never touches the schema. Data-retention rules are
//! one such setting (see [`RetentionSettings`]).

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::retention::RetentionSettings;

/// Whether processed files require approval before becoming visible, on by
/// default (the safe choice for a redaction workflow).
#[must_use]
const fn default_require_approval() -> bool {
    true
}

/// Typed workspace settings, the JSON stored in the `workspaces.settings` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkspaceSettings {
    /// Whether approval is required before processed files become visible.
    pub require_approval: bool,
    /// Data-retention rules for the workspace.
    pub retention: RetentionSettings,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            require_approval: default_require_approval(),
            retention: RetentionSettings::default(),
        }
    }
}

impl WorkspaceSettings {
    /// Parses typed settings from the stored JSONB, treating an absent or
    /// malformed blob as the default so a bad blob never fails a read.
    #[must_use]
    pub fn from_value(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    /// Serializes to a JSON value for the `settings` column.
    ///
    /// # Errors
    ///
    /// Returns an error only if serialization fails, which cannot happen for
    /// this type.
    pub fn to_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn empty_settings_blob_is_default() {
        let settings = WorkspaceSettings::from_value(&json!({}));
        assert!(settings.require_approval);
        assert!(settings.retention.is_noop());
    }

    #[test]
    fn malformed_settings_blob_falls_back_to_default() {
        let settings = WorkspaceSettings::from_value(&json!({ "retention": "nonsense" }));
        assert!(settings.require_approval);
        assert!(settings.retention.is_noop());
    }

    #[test]
    fn require_approval_survives_round_trip() {
        let settings = WorkspaceSettings {
            require_approval: false,
            ..Default::default()
        };
        let value = settings.to_value().expect("serialize");
        assert_eq!(WorkspaceSettings::from_value(&value), settings);
    }
}
