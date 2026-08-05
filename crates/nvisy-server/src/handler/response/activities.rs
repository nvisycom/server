//! Workspace activity response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceActivity;
use nvisy_postgres::types::{ActivityType, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a workspace activity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Unique activity identifier.
    pub id: Uuid,
    /// Handle of the workspace this activity belongs to.
    pub workspace_slug: Handle,
    /// Account that performed the activity.
    pub performed_by: AccountRef,
    /// Type of activity.
    pub activity_type: ActivityType,
    /// Human-readable description.
    pub description: String,
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
            activity_type: activity.activity_type,
            description: activity.description,
            created_at: activity.created_at.into(),
        }
    }
}
