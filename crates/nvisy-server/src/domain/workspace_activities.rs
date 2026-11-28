//! Workspace activity domain service: the activity log's paged list and its
//! capped export.

use nvisy_postgres::PgClient;
use nvisy_postgres::model::WorkspaceActivity;
use nvisy_postgres::query::{ActivityFilter, WorkspaceActivityRepository};
use nvisy_postgres::types::{CursorPage, CursorPagination, TimeCursor, WithAccountRef};
use uuid::Uuid;

use crate::response::Result;

/// Tracing target for activity domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::activities";

/// The activities an export returns, oldest first, cut at the row cap.
#[derive(Debug)]
pub struct ActivityExport {
    /// The exported activities, each with its actor.
    pub rows: Vec<WithAccountRef<WorkspaceActivity>>,
    /// Whether the window held more than the cap, so its newest rows were
    /// dropped.
    pub truncated: bool,
}

/// Reads a workspace's activity log.
#[derive(Clone)]
pub struct WorkspaceActivityService {
    postgres: PgClient,
}

impl WorkspaceActivityService {
    /// Creates a [`WorkspaceActivityService`] over the connection pool.
    #[must_use]
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Lists a workspace's activities matching `filter`, newest first.
    ///
    /// # Errors
    ///
    /// - A database error if a connection or the query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        pagination: CursorPagination<TimeCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceActivity>>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_activity(workspace_id, filter, pagination)
            .await?)
    }

    /// Loads a workspace's activities matching `filter` for export, oldest
    /// first, keeping at most `cap` rows. Rows past the cap are the newest in the
    /// window; the result says whether any were dropped.
    ///
    /// # Errors
    ///
    /// - A database error if a connection or the query fails.
    pub async fn export(
        &self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        cap: usize,
    ) -> Result<ActivityExport> {
        let mut conn = self.postgres.get_connection().await?;
        // Fetch one past the cap so a full result signals truncation.
        let fetch_limit = i64::try_from(cap.saturating_add(1)).unwrap_or(i64::MAX);
        let mut rows = conn
            .list_workspace_activity_for_export(workspace_id, filter, fetch_limit)
            .await?;

        let truncated = rows.len() > cap;
        rows.truncate(cap);
        if truncated {
            tracing::warn!(
                target: TRACING_TARGET,
                cap,
                "Activity export hit the row cap; the newest rows in the window were dropped"
            );
        }
        Ok(ActivityExport { rows, truncated })
    }
}
