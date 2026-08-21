//! Shared request types for file exports.

use schemars::JsonSchema;
use serde::Deserialize;

/// The file format an export is rendered as.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// Comma-separated values (the default).
    #[default]
    Csv,
    /// JSON.
    Json,
}

/// Query parameters for an export whose only choice is the output format. Used by
/// endpoints that already scope their data another way (e.g. by path), so the
/// format is all the query carries.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportQuery {
    /// Output format; defaults to `csv`.
    #[serde(default)]
    pub format: ExportFormat,
}
