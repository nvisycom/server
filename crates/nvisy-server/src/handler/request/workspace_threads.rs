//! Thread request types (open a thread, rename it, address it by id, filter the
//! listing).

use garde::Validate;
use nvisy_postgres::types::ThreadFilter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::validators::validate_non_blank;
use crate::handler::request::CommentAnchor;

/// Path parameters addressing one thread by its opaque id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPathParams {
    /// Unique identifier of the thread.
    pub thread_id: Uuid,
}

/// Request payload to open a comment thread with its first message.
///
/// A thread pins a discussion to a location within a file (`anchor`), to a file
/// as a whole (no anchor), or — when opened on the workspace endpoint — to no
/// file at all. `@username` mentions in the opening body notify those members.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OpenThread {
    /// Optional title for the thread (1-255 characters). Omit for an untitled
    /// thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(inner(length(chars, min = 1, max = 255), custom(validate_non_blank)))]
    pub display_name: Option<String>,
    /// The opening message text (1-10000 characters).
    #[garde(length(chars, min = 1, max = 10_000), custom(validate_non_blank))]
    pub body: String,
    /// Locations within the file the thread is pinned to. Empty for a file-level
    /// thread (no pin). Ignored for a workspace-level thread (no file).
    ///
    /// The `max = 32` limit is the same cap `add_anchor` enforces per thread
    /// (`nvisy_postgres::query::MAX_THREAD_ANCHORS`); keep the two in sync.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(length(max = 32))]
    pub anchors: Vec<CommentAnchor>,
}

/// Request payload to rename a thread (set or clear its title).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RenameThread {
    /// The new title (1-255 characters), or `null` to clear it. Omitting the
    /// field leaves the current title unchanged; only an explicit `null` clears
    /// it. The outer `Option` distinguishes "absent" (`None`) from "explicit
    /// null" (`Some(None)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(inner(inner(length(chars, min = 1, max = 255), custom(validate_non_blank))))]
    pub display_name: Option<Option<String>>,
}

/// Query parameters for listing a workspace's threads.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceThreadsQuery {
    /// Filter by the file the thread is pinned to.
    pub file_id: Option<Uuid>,
    /// Filter by the thread's opening author.
    pub author: Option<Uuid>,
    /// Filter by open/closed state: `true` = closed only, `false` = open only.
    pub closed: Option<bool>,
}

impl From<WorkspaceThreadsQuery> for ThreadFilter {
    fn from(query: WorkspaceThreadsQuery) -> Self {
        ThreadFilter {
            file_id: query.file_id,
            author_account_id: query.author,
            closed: query.closed,
        }
    }
}
