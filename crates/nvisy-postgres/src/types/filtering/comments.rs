//! Filtering options for comment queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Filter options for workspace comments.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The workspace scope is applied by the query itself, not carried here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommentFilter {
    /// Filter by the file the comment is on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<Uuid>,
    /// Filter by the comment's author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_account_id: Option<Uuid>,
    /// Filter by resolution state: `Some(true)` = resolved only, `Some(false)` =
    /// open only, `None` = either.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
}
