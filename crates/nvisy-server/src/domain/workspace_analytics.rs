//! Workspace analytics domain logic: the dashboard snapshot and the daily
//! detection series.

use jiff::Timestamp;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{AnalyticsSnapshot, DetectionDayPoint, WorkspaceAnalyticsRepository};
use uuid::Uuid;

use crate::response::Result;

/// Reads a workspace's aggregate analytics.
///
/// Holds the Postgres client, acquiring its own connection per call. Resolved per
/// request from [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceAnalyticsService {
    postgres: PgClient,
}

impl WorkspaceAnalyticsService {
    /// Creates a [`WorkspaceAnalyticsService`] over the Postgres client.
    #[must_use]
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// A workspace's stored-file and detection-health snapshot.
    ///
    /// # Errors
    ///
    /// - A database error if the connection or query fails.
    pub async fn snapshot(&self, workspace_id: Uuid) -> Result<AnalyticsSnapshot> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn.snapshot(workspace_id).await?)
    }

    /// A workspace's detection activity per day between `from` and `to`.
    ///
    /// # Errors
    ///
    /// - A database error if the connection or query fails.
    pub async fn detections_by_day(
        &self,
        workspace_id: Uuid,
        from: Timestamp,
        to: Timestamp,
    ) -> Result<Vec<DetectionDayPoint>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn.detections_by_day(workspace_id, from, to).await?)
    }
}
