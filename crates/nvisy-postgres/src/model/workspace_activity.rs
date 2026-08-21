//! Workspace activity model for PostgreSQL database operations.
//!
//! This module provides models for tracking and managing workspace activity logs,
//! which record all significant actions performed within workspaces.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_activities;
use crate::types::{ActivityPayload, ActivityType, HasCreatedAt, HasSecurityContext, Json};

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
    pub account_id: Uuid,
    /// Type of activity performed.
    pub activity_type: ActivityType,
    /// The self-describing tagged payload (its `activityType` + params).
    pub params: Json<ActivityPayload>,
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
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_activities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceActivity {
    /// Reference to the workspace where the activity occurred.
    pub workspace_id: Uuid,
    /// Reference to the account that performed the activity.
    pub account_id: Uuid,
    /// Type of activity being logged.
    pub activity_type: ActivityType,
    /// The self-describing tagged payload (its `activityType` + params).
    pub params: Json<ActivityPayload>,
    /// IP address of the client that initiated the activity.
    pub ip_address: Option<IpNet>,
    /// User agent string from the client request.
    pub user_agent: Option<String>,
}

impl WorkspaceActivity {
    /// Returns whether the activity carries typed params.
    pub fn has_params(&self) -> bool {
        !self.params.is_empty()
    }

    /// Returns whether the activity has location information.
    pub fn has_location_info(&self) -> bool {
        self.ip_address.is_some()
    }

    /// Returns whether the activity has user agent information.
    pub fn has_user_agent(&self) -> bool {
        self.user_agent.as_deref().is_some_and(|ua| !ua.is_empty())
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
