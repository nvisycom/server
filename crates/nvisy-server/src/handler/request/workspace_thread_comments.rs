//! Comment request types (post and edit a message, address a comment by id).

use garde::Validate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::validators::validate_non_blank;

/// Path parameters addressing one comment by its opaque id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCommentPathParams {
    /// Unique identifier of the comment.
    pub comment_id: Uuid,
}

/// Request payload to post a comment (message) in a thread.
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
