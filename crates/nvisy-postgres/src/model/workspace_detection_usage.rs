//! Per-model inference usage for a detection.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_detection_usage;

/// One model's token usage within a detection, as the provider reported it.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = workspace_detection_usage)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDetectionUsage {
    /// Unique usage row identifier.
    pub id: Uuid,
    /// The detection this usage belongs to.
    pub detection_id: Uuid,
    /// The model the recognizers used.
    pub model: String,
    /// The model version, if the provider reported one.
    pub version: Option<String>,
    /// Input/prompt tokens for this model; `None` if not reported.
    pub input_tokens: Option<i64>,
    /// Output/completion tokens for this model; `None` if not reported.
    pub output_tokens: Option<i64>,
    /// Total tokens as reported (not necessarily input + output); `None` if not
    /// reported.
    pub total_tokens: Option<i64>,
    /// Wall-clock time this model spent, in milliseconds.
    pub duration_ms: i64,
}

/// Data for recording one model's usage on a detection.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_detection_usage)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceDetectionUsage {
    /// The detection this usage belongs to.
    pub detection_id: Uuid,
    /// The model the recognizers used.
    pub model: String,
    /// The model version, if any.
    pub version: Option<String>,
    /// Input/prompt tokens; `None` if not reported.
    pub input_tokens: Option<i64>,
    /// Output/completion tokens; `None` if not reported.
    pub output_tokens: Option<i64>,
    /// Total tokens as reported; `None` if not reported.
    pub total_tokens: Option<i64>,
    /// Wall-clock time this model spent, in milliseconds.
    pub duration_ms: i64,
}

impl NewWorkspaceDetectionUsage {
    /// A usage row for `detection_id` on `model`, reporting the given token
    /// totals, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(detection_id: Uuid, model: impl Into<String>, input: i64, output: i64) -> Self {
        Self {
            detection_id,
            model: model.into(),
            version: None,
            input_tokens: Some(input),
            output_tokens: Some(output),
            total_tokens: Some(input + output),
            duration_ms: 100,
        }
    }
}
