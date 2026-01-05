//! Workspace activity model for PostgreSQL database operations.
//!
//! This module provides models for tracking and managing workspace activity logs,
//! which record all significant actions performed within workspaces.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_activities;
use crate::types::{ActivityCategory, ActivityType, HasCreatedAt, HasSecurityContext};

/// Workspace activity log entry representing an action performed in a workspace.
///
/// This model captures comprehensive information about activities performed within
/// a workspace, including user actions, system events, and security-related activities.
/// Activity entries are immutable once created and provide a complete historical
/// record of workspace changes and interactions.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_activities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceActivity {
    /// Unique activity log entry identifier.
    pub id: Uuid,
    /// Reference to the workspace where activity occurred.
    pub workspace_id: Uuid,
    /// Reference to the account that performed the activity.
    pub account_id: Option<Uuid>,
    /// Type of activity performed.
    pub activity_type: ActivityType,
    /// Human-readable description of the activity.
    pub description: String,
    /// Additional structured metadata about the activity.
    pub metadata: serde_json::Value,
    /// IP address from which the activity originated.
    pub ip_address: Option<IpNet>,
    /// User agent string of the client that performed the activity.
    pub user_agent: Option<String>,
    /// Timestamp when the activity occurred.
    pub created_at: Timestamp,
}

/// Data structure for creating a new workspace activity entry.
///
/// Contains all the necessary information to log a new activity in the workspace
/// activity log. The ID and creation timestamp are automatically generated.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_activities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceActivity {
    /// Reference to the workspace where the activity occurred.
    pub workspace_id: Uuid,
    /// Reference to the account that performed the activity.
    pub account_id: Option<Uuid>,
    /// Type of activity being logged.
    pub activity_type: ActivityType,
    /// Human-readable description of what occurred.
    pub description: Option<String>,
    /// Additional structured data about the activity.
    pub metadata: Option<serde_json::Value>,
    /// IP address of the client that initiated the activity.
    pub ip_address: Option<IpNet>,
    /// User agent string from the client request.
    pub user_agent: Option<String>,
}

impl WorkspaceActivity {
    /// Returns whether this activity was performed by a system process.
    pub fn is_system_activity(&self) -> bool {
        self.account_id.is_none()
    }

    /// Returns whether this activity was performed by a user.
    pub fn is_user_activity(&self) -> bool {
        self.account_id.is_some()
    }

    /// Returns whether the activity has additional metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the activity has location information.
    pub fn has_location_info(&self) -> bool {
        self.ip_address.is_some()
    }

    /// Returns whether the activity has user agent information.
    pub fn has_user_agent(&self) -> bool {
        self.user_agent.as_deref().is_some_and(|ua| !ua.is_empty())
    }

    /// Returns the high-level category of this activity.
    pub fn category(&self) -> ActivityCategory {
        self.activity_type.category()
    }

    /// Returns whether this is a high-priority activity requiring attention.
    pub fn is_high_priority(&self) -> bool {
        matches!(
            self.category(),
            ActivityCategory::Member | ActivityCategory::Workspace
        )
    }

    /// Returns whether this represents a direct user action.
    pub fn is_user_action(&self) -> bool {
        matches!(
            self.category(),
            ActivityCategory::Member | ActivityCategory::Document
        )
    }

    /// Returns whether this represents a system event.
    pub fn is_system_event(&self) -> bool {
        matches!(
            self.category(),
            ActivityCategory::Workspace | ActivityCategory::Integration
        )
    }

    /// Returns a truncated summary of the activity description.
    pub fn summary(&self) -> String {
        if self.description.len() > 100 {
            format!("{}...", &self.description[..97])
        } else {
            self.description.clone()
        }
    }

    /// Returns whether this activity requires special audit attention.
    pub fn requires_audit(&self) -> bool {
        self.is_high_priority() || matches!(self.category(), ActivityCategory::Member)
    }

    /// Returns the IP address as a formatted string.
    pub fn ip_address_string(&self) -> Option<String> {
        self.ip_address.map(|ip| ip.to_string())
    }
}

impl HasCreatedAt for WorkspaceActivity {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasSecurityContext for WorkspaceActivity {
    fn ip_address(&self) -> Option<IpNet> {
        self.ip_address
    }

    fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }
}
