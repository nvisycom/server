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
    #[serde(alias = "force")]
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
    #[serde(alias = "ocr")]
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

impl WorkspaceSettings {
    /// The effective per-file upload limit in bytes: the smaller of this
    /// workspace's own soft cap (the hard limit when it set none) and the
    /// server-wide `hard_max_upload_bytes`. Always a concrete number a client can
    /// enforce.
    #[must_use]
    pub fn effective_max_upload_bytes(&self, hard_max_upload_bytes: u64) -> u64 {
        self.max_upload_bytes.map_or(hard_max_upload_bytes, |soft| {
            soft.min(hard_max_upload_bytes)
        })
    }

    /// Returns these settings with `max_upload_bytes` replaced by the effective
    /// per-file limit for `hard_max_upload_bytes`, so a response always exposes a
    /// single concrete cap a client can enforce rather than the raw soft value.
    #[must_use]
    pub fn resolved(mut self, hard_max_upload_bytes: u64) -> Self {
        self.max_upload_bytes = Some(self.effective_max_upload_bytes(hard_max_upload_bytes));
        self
    }
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

    #[test]
    fn legacy_ocr_field_and_force_variant_still_deserialize() {
        // Settings persisted before the `ocr`/`force` rename must keep their
        // behaviour: the `ocr` key aliases `raster`, and `force` aliases `always`.
        let settings = column(json!({ "ocr": "force" })).or_default();
        assert_eq!(settings.raster, RasterPolicy::Always);

        let settings = column(json!({ "ocr": "never" })).or_default();
        assert_eq!(settings.raster, RasterPolicy::Never);
    }

    #[test]
    fn effective_max_upload_bytes_is_the_smaller_of_soft_and_hard() {
        let hard = 12 * 1024 * 1024;

        // No soft cap: the hard limit governs.
        let settings = WorkspaceSettings::default();
        assert_eq!(settings.effective_max_upload_bytes(hard), hard);

        // Soft cap below the hard limit wins.
        let settings = WorkspaceSettings {
            max_upload_bytes: Some(8 * 1024 * 1024),
            ..Default::default()
        };
        assert_eq!(settings.effective_max_upload_bytes(hard), 8 * 1024 * 1024);

        // Soft cap above the hard limit is clamped to it.
        let settings = WorkspaceSettings {
            max_upload_bytes: Some(64 * 1024 * 1024),
            ..Default::default()
        };
        assert_eq!(settings.effective_max_upload_bytes(hard), hard);
    }
}
