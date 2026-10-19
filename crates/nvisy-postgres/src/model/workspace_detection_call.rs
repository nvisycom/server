//! Per-call inference usage for a detection: one model call and its meters.

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{workspace_detection_call, workspace_detection_call_meter};
use crate::types::UsageMeter;

/// Data for recording one model call made during a detection.
///
/// One row per call the engine reports: calls are not collapsed by model, so a
/// model called several times keeps a row (and its own outcome and duration) per
/// call.
///
/// The `id` is assigned client-side at write time rather than defaulted by the
/// database, so a call's meter rows can reference it in the same batch without a
/// round-trip to read the generated key back (see `record_detection_usage`).
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_detection_call)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceDetectionCall {
    /// This call's identifier, assigned before insert so meters can reference it.
    pub id: Uuid,
    /// The detection this call belongs to.
    pub detection_id: Uuid,
    /// The model the call used.
    pub model: String,
    /// The model version, if the provider reported one.
    pub version: Option<String>,
    /// How the call ended: `ok`, or the engine's error-kind label.
    pub outcome: String,
    /// This call's wall-clock round-trip, in milliseconds.
    pub duration_ms: i64,
    /// Items the call produced, if the response reported a count.
    pub output_count: Option<i64>,
}

/// Data for recording one billing meter a call spent.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_detection_call_meter)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceDetectionCallMeter {
    /// The call this meter belongs to.
    pub call_id: Uuid,
    /// Which unit was metered.
    pub meter: UsageMeter,
    /// How much of the unit was spent; meaningful only together with `meter`.
    pub amount: BigDecimal,
}

impl NewWorkspaceDetectionCall {
    /// A successful call of `model` on `detection_id`, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(detection_id: Uuid, model: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            detection_id,
            model: model.into(),
            version: None,
            outcome: "ok".to_owned(),
            duration_ms: 100,
            output_count: None,
        }
    }
}

/// One model call together with the meters it spent.
///
/// The meters carry no `call_id`: each is paired with the call's own `id` at
/// write time, so the call and its meters insert in one batch without reading a
/// generated key back.
#[must_use]
pub struct DetectionCallRecord {
    /// The call to record.
    pub call: NewWorkspaceDetectionCall,
    /// The meters the call spent: `(unit, amount)` pairs, possibly empty (a call
    /// that billed nothing, e.g. a failure).
    pub meters: Vec<(UsageMeter, BigDecimal)>,
}
