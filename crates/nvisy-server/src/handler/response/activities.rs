//! Workspace activity response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceActivity;
use nvisy_postgres::types::ActivityType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for a workspace activity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Unique activity identifier.
    pub id: Uuid,
    /// Workspace ID.
    pub workspace_id: Uuid,
    /// Account that performed the activity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
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
    pub fn from_model(activity: WorkspaceActivity) -> Self {
        Self {
            id: activity.id,
            workspace_id: activity.workspace_id,
            account_id: activity.account_id,
            activity_type: activity.activity_type,
            description: activity.description,
            created_at: activity.created_at.into(),
        }
    }
}
