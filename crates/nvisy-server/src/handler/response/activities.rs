//! Workspace activity response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceActivity;
use nvisy_postgres::types::{ActivityPayload, Handle};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a workspace activity.
///
/// The typed payload is nested under `payload`, so an activity is
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
    /// The activity type and its typed params, absent when the stored params do
    /// not decode into their `activityType`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<ActivityPayload>,
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
            payload: activity.params.optional(),
            created_at: activity.created_at.into(),
        }
    }
}
