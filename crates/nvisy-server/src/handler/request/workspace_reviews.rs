//! Review request types: open a review, rename it, address one by id, post and
//! edit comments, link artifacts, assign/unassign, and filter the review queue.

use garde::Validate;
use nvisy_postgres::types::{DocumentReviewFilter, ReviewStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::validators::validate_non_blank;

/// Path parameters addressing one review by its id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReviewPathParams {
    /// Unique identifier of the review.
    pub review_id: Uuid,
}

/// Path parameters addressing a review-to-detection link.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReviewDetectionPathParams {
    /// Unique identifier of the review.
    pub review_id: Uuid,
    /// Unique identifier of the detection to reference.
    pub detection_id: Uuid,
}

/// Path parameters addressing a review-to-redaction link.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReviewRedactionPathParams {
    /// Unique identifier of the review.
    pub review_id: Uuid,
    /// Unique identifier of the redaction to reference.
    pub redaction_id: Uuid,
}

/// Request payload to open a review on a document.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceReview {
    /// The review's title (1-255 characters).
    #[garde(length(chars, min = 1, max = 255), custom(validate_non_blank))]
    pub display_name: String,
}

/// Request payload to rename a review (set its title).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RenameWorkspaceReview {
    /// The new title (1-255 characters). Omitting the field leaves the current
    /// title unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(inner(length(chars, min = 1, max = 255), custom(validate_non_blank)))]
    pub display_name: Option<String>,
}

/// Path parameters addressing a reviewer assignment on a review.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReviewAssigneePathParams {
    /// Unique identifier of the review.
    pub review_id: Uuid,
    /// Account id of the reviewer to assign or unassign.
    pub account_id: Uuid,
}

/// Path parameters addressing one comment by its id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCommentPathParams {
    /// Unique identifier of the comment.
    pub comment_id: Uuid,
}

/// Request payload to post a comment (message) in a review.
///
/// `@username` mentions in the body notify those workspace members.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceComment {
    /// The comment text (1-10000 characters).
    #[garde(length(chars, min = 1, max = 10_000), custom(validate_non_blank))]
    pub body: String,
}

/// Request payload to edit a comment's body.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceComment {
    /// The new comment text (1-10000 characters).
    #[garde(length(chars, min = 1, max = 10_000), custom(validate_non_blank))]
    pub body: String,
}

/// Query parameters for listing a workspace's reviews (the review queue).
///
/// Every field is an optional filter; unset fields impose no constraint. Accounts
/// are addressed by id.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReviewsQuery {
    /// Filter to the reviews of a specific document.
    pub document_id: Option<Uuid>,
    /// Filter by the review's opening author (account id).
    pub author: Option<Uuid>,
    /// Filter by the assigned reviewer (account id).
    pub assignee: Option<Uuid>,
    /// Filter by review status.
    pub review_status: Option<ReviewStatus>,
}

impl WorkspaceReviewsQuery {
    /// Builds the repository filter. All fields are ids passed straight through; a
    /// nonexistent id simply matches no rows.
    #[must_use]
    pub fn into_filter(self) -> DocumentReviewFilter {
        DocumentReviewFilter {
            document_id: self.document_id,
            author_account_id: self.author,
            assignee_account_id: self.assignee,
            review_status: self.review_status,
        }
    }
}
