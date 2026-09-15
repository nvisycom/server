//! Filtering options for thread and document-review queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::ReviewStatus;

/// Filter options for workspace discussion threads.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The workspace scope is applied by the query itself, not carried here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ThreadFilter {
    /// Filter by the account that opened the thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_account_id: Option<Uuid>,
    /// Filter by open/closed state: `Some(true)` = closed only, `Some(false)` =
    /// open only, `None` = either.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
}

/// Filter options for document reviews.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The workspace scope is applied by the query itself, not carried here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DocumentReviewFilter {
    /// Filter to the review of a specific document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<Uuid>,
    /// Filter by the assigned reviewer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_account_id: Option<Uuid>,
    /// Filter by review status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_status: Option<ReviewStatus>,
}
