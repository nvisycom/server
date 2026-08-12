//! Workspace activity response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceActivity;
use nvisy_postgres::types::{ActivityPayload, Handle, TypedBody};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::{AccountRef, Page};

/// The rendered body of an activity: the typed payload when the stored params
/// decode into their `activityType`, or a raw fallback when they do not.
pub type ActivityBody = TypedBody<ActivityPayload>;

/// Response type for a workspace activity.
///
/// The [`ActivityBody`] is nested under `payload`, so an activity is
/// `{ id, workspaceSlug, performedBy, payload: { activityType, <params...> }, createdAt }`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Unique activity identifier.
    pub id: Uuid,
    /// Handle of the workspace this activity belongs to.
    pub workspace_slug: Handle,
    /// Account that performed the activity.
    pub performed_by: AccountRef,
    /// The activity type and its typed params (or a raw fallback).
    pub payload: ActivityBody,
    /// When the activity occurred.
    pub created_at: Timestamp,
}

/// Paginated list of activities.
pub type ActivitiesPage = Page<Activity>;

impl Activity {
    pub fn from_model(
        activity: WorkspaceActivity,
        workspace_slug: Handle,
        performed_by: AccountRef,
    ) -> Self {
        Self {
            id: activity.id,
            workspace_slug,
            performed_by,
            payload: activity.params.decode(),
            created_at: activity.created_at.into(),
        }
    }
}
