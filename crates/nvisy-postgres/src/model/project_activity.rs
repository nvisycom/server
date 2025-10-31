//! Project activity model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::project_activity_log;

/// Project activity log entry representing an action performed in a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_activity_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectActivity {
    /// Unique activity log entry identifier
    pub id: i64,
    /// Reference to the project where activity occurred
    pub project_id: Uuid,
    /// Reference to the account that performed the activity (NULL for system actions)
    pub actor_id: Option<Uuid>,
    /// Type of activity performed
    pub activity_type: String,
    /// Description of the activity
    pub description: String,
    /// Additional metadata (JSON)
    pub metadata: serde_json::Value,
    /// IP address from which the activity originated
    pub ip_address: Option<IpNet>,
    /// User agent of the client that performed the activity
    pub user_agent: Option<String>,
    /// Timestamp when the activity occurred
    pub created_at: OffsetDateTime,
}

/// Data for creating a new project activity log entry.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = project_activity_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectActivity {
    /// Project ID
    pub project_id: Uuid,
    /// Actor ID
    pub actor_id: Option<Uuid>,
    /// Activity type
    pub activity_type: String,
    /// Description
    pub description: String,
    /// Metadata
    pub metadata: serde_json::Value,
    /// IP address
    pub ip_address: Option<IpNet>,
    /// User agent
    pub user_agent: Option<String>,
}

impl Default for NewProjectActivity {
    fn default() -> Self {
        Self {
            project_id: Uuid::new_v4(),
            actor_id: None,
            activity_type: String::new(),
            description: String::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            ip_address: None,
            user_agent: None,
        }
    }
}

impl ProjectActivity {
    /// Returns whether this activity was performed by a user.
    pub fn has_actor(&self) -> bool {
        self.actor_id.is_some()
    }

    /// Returns whether this is a system-generated activity.
    pub fn is_system_activity(&self) -> bool {
        self.actor_id.is_none()
    }

    /// Returns whether this activity has metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether this activity includes client context (IP and user agent).
    pub fn has_client_context(&self) -> bool {
        self.ip_address.is_some() || self.user_agent.is_some()
    }

    /// Returns whether this activity occurred recently (within last hour).
    pub fn is_recent(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let duration = now - self.created_at;
        duration.whole_hours() < 1
    }

    /// Returns whether this activity occurred today.
    pub fn is_today(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let activity_date = self.created_at.date();
        let today = now.date();
        activity_date == today
    }
}
