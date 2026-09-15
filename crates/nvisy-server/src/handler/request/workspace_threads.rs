//! Thread request types (open a thread, rename it, address it by id, filter the
//! listing).

use garde::Validate;
use nvisy_postgres::types::ThreadFilter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::input::OpenThreadInput;
use crate::extract::validators::validate_non_blank;

/// Path parameters addressing one thread by its opaque id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceThreadPathParams {
    /// Unique identifier of the thread.
    pub thread_id: Uuid,
}

/// Request payload to open a workspace discussion thread with its first message.
///
/// A workspace thread is free-form discussion pinned to no document; document
/// reviews are auto-created on detection, not opened by hand. `@username`
/// mentions in the opening body notify those members.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceThread {
    /// Optional title for the thread (1-255 characters). Omit for an untitled
    /// thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(inner(length(chars, min = 1, max = 255), custom(validate_non_blank)))]
    pub display_name: Option<String>,
    /// The opening message text (1-10000 characters).
    #[garde(length(chars, min = 1, max = 10_000), custom(validate_non_blank))]
    pub body: String,
}

impl From<OpenWorkspaceThread> for OpenThreadInput {
    fn from(request: OpenWorkspaceThread) -> Self {
        OpenThreadInput {
            display_name: request.display_name,
            body: request.body,
        }
    }
}

/// Request payload to rename a thread (set or clear its title).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RenameWorkspaceThread {
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
/// Every field is an optional filter; unset fields impose no constraint. Accounts
/// are addressed by id.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceThreadsQuery {
    /// Filter by the thread's opening author (account id).
    pub author: Option<Uuid>,
    /// Filter by open/closed state: `true` = closed only, `false` = open only.
    pub closed: Option<bool>,
}

impl WorkspaceThreadsQuery {
    /// Builds the repository filter. All fields are ids passed straight through; a
    /// nonexistent id simply matches no rows.
    #[must_use]
    pub fn into_filter(self) -> ThreadFilter {
        ThreadFilter {
            author_account_id: self.author,
            closed: self.closed,
        }
    }
}
