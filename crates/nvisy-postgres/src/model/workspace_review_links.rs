//! Review link models: the many-to-many rows tying a review to the detections and
//! redactions it references for its purpose.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{workspace_review_detections, workspace_review_redactions};

/// A link from a review to a detection it references.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_review_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewReviewDetection {
    /// The referencing review.
    pub review_id: Uuid,
    /// The referenced detection.
    pub detection_id: Uuid,
}

/// A link from a review to a redaction it references.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_review_redactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewReviewRedaction {
    /// The referencing review.
    pub review_id: Uuid,
    /// The referenced redaction.
    pub redaction_id: Uuid,
}
