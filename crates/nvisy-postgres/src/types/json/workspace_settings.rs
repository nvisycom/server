//! Typed workspace settings: the structured view of the `workspaces.settings`
//! JSONB column.
//!
//! Every configurable workspace preference lives here rather than as its own
//! column, so adding a setting never touches the schema. Data-retention rules are
//! one such setting (see [`RetentionSettings`]).

use serde::{Deserialize, Serialize};

use super::retention::RetentionSettings;

/// How a workspace's document pages are rasterised to images for OCR during
/// detection.
///
/// A workspace-level policy over the engine's per-run raster mode: `Auto` lets
/// the engine decide from the text layer, `Always` renders every page (for
/// documents with unreliable text layers — scans, watermarks), and `Never`
/// relies on the text layer only.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RasterPolicy {
    /// Extract the text layer where present and rasterise only pages that lack it.
    #[default]
    Auto,
    /// Always render every page to images, ignoring any text layer.
    Always,
    /// Rely on the text layer only; never rasterise pages.
    Never,
}

/// Typed workspace settings, the JSON stored in the `workspaces.settings` column.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkspaceSettings {
    /// How document pages are rasterised for OCR during detection.
    pub raster: RasterPolicy,
    /// Data-retention rules for the workspace.
    pub retention: RetentionSettings,
    /// A soft per-file upload cap in bytes: an upload larger than this is
    /// rejected for this workspace. `None` imposes no workspace-specific cap.
    ///
    /// The server-wide hard limit still applies regardless; the effective cap is
    /// the smaller of the two.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_upload_bytes: Option<u64>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::types::Json;

    /// Wraps a raw JSON value as a settings column for `or_default` testing.
    fn column(value: serde_json::Value) -> Json<WorkspaceSettings> {
        Json::from_raw(value)
    }

    #[test]
    fn empty_settings_blob_is_default() {
        let settings = column(json!({})).or_default();
        assert_eq!(settings.raster, RasterPolicy::Auto);
        assert!(settings.retention.is_noop());
    }

    #[test]
    fn malformed_settings_blob_falls_back_to_default() {
        let settings = column(json!({ "retention": "nonsense" })).or_default();
        assert_eq!(settings.raster, RasterPolicy::Auto);
        assert!(settings.retention.is_noop());
    }

    #[test]
    fn raster_policy_defaults_to_auto_and_round_trips() {
        assert_eq!(column(json!({})).or_default().raster, RasterPolicy::Auto);
        let settings = WorkspaceSettings {
            raster: RasterPolicy::Always,
            ..Default::default()
        };
        assert_eq!(Json::encode(&settings).or_default(), settings);
    }

    #[test]
    fn max_upload_bytes_round_trips() {
        let settings = WorkspaceSettings {
            max_upload_bytes: Some(25 * 1024 * 1024),
            ..Default::default()
        };
        assert_eq!(Json::encode(&settings).or_default(), settings);
    }
}
