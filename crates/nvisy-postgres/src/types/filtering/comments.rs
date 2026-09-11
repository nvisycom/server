//! Filtering options for comment-thread queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::ReviewStatus;

/// Filter options for workspace comment threads.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The workspace scope is applied by the query itself, not carried here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ThreadFilter {
    /// Filter by the file the thread reviews.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<Uuid>,
    /// Filter by the account that opened the thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_account_id: Option<Uuid>,
    /// Filter a workspace thread by open/closed state: `Some(true)` = closed only,
    /// `Some(false)` = open only, `None` = either.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    /// Filter file review threads by their review status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_status: Option<ReviewStatus>,
}
