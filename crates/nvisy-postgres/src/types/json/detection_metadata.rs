//! Structured metadata for the `workspace_detections.metadata` JSONB column.
//!
//! A column the server itself populates has a known shape, so it is typed rather
//! than left as free-form JSON. Read with `Json::or_default` so an absent or
//! older blob yields an empty value rather than failing the read.

use serde::{Deserialize, Serialize};

/// Structured metadata for a detection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct DetectionMetadata {
    /// Free-form labels attached to the detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Failure reason recorded when the detection failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The engine's full per-recognizer usage report (durations, per-model token
    /// counts), stored opaquely for drill-down. Per-model token totals for
    /// aggregation live in the `workspace_detection_usage` table; this keeps the
    /// detail. Absent when the detection produced no usage.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<serde_json::Value>"))]
    pub usage: Option<serde_json::Value>,
}
