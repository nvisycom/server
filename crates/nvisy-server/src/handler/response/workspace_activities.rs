//! Workspace activity response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceActivity as WorkspaceActivityModel;
use nvisy_postgres::types::ActivityPayload;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::{AccountRef, Page, WorkspaceRef};

/// Response type for a workspace activity.
///
/// The typed payload is nested under `payload`, so an activity is
/// `{ id, workspaceHandle, performedBy, payload: { activityType, <params...> }, createdAt }`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceActivity {
    /// Unique activity identifier.
    pub id: Uuid,
    /// The workspace it belongs to.
    #[serde(flatten)]
    pub workspace: WorkspaceRef,
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
pub type WorkspaceActivitiesPage = Page<WorkspaceActivity>;

impl WorkspaceActivity {
    #[must_use]
    pub fn from_model(
        activity: &WorkspaceActivityModel,
        workspace: WorkspaceRef,
        performed_by: AccountRef,
    ) -> Self {
        Self {
            id: activity.id,
            workspace,
            performed_by,
            payload: activity.params.optional(),
            created_at: activity.created_at.into(),
        }
    }
}
