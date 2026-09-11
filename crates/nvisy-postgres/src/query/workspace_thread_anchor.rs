//! Workspace thread-anchor repository: a thread's location pins, added and
//! removed over its lifetime. Each add/remove records a timeline event snapshot
//! so the timeline renders the anchor even after its row is soft-removed.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use super::workspace_thread_event::anchor_snapshot;
use crate::model::{NewWorkspaceThreadAnchor, NewWorkspaceThreadEvent, WorkspaceThreadAnchor};
use crate::types::ThreadEventKind;
use crate::{AsyncConnection, Error, PgConnection, Result, schema};

/// Read and write operations on a thread's anchors.
pub trait WorkspaceThreadAnchorRepository {
    /// Adds an anchor to a thread, recording an `anchor.added` timeline event, in
    /// one transaction. Returns the created anchor.
    fn add_thread_anchor(
        &mut self,
        workspace_id: Uuid,
        new_anchor: NewWorkspaceThreadAnchor,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThreadAnchor>> + Send;

    /// Soft-removes an anchor, recording an `anchor.removed` timeline event, in
    /// one transaction. Returns the removed anchor.
    fn remove_thread_anchor(
        &mut self,
        workspace_id: Uuid,
        anchor_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThreadAnchor>> + Send;

    /// Finds a live anchor by id within a thread.
    fn find_thread_anchor(
        &mut self,
        thread_id: Uuid,
        anchor_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceThreadAnchor>>> + Send;

    /// Lists a thread's live anchors, oldest first.
    fn list_thread_anchors(
        &mut self,
        thread_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WorkspaceThreadAnchor>>> + Send;

    /// Lists the live anchors of several threads in one query, ordered by thread
    /// then creation. The caller groups them by `thread_id`; a thread with no
    /// anchors is simply absent from the result.
    fn list_anchors_for_threads(
        &mut self,
        thread_ids: &[Uuid],
    ) -> impl Future<Output = Result<Vec<WorkspaceThreadAnchor>>> + Send;
}

impl WorkspaceThreadAnchorRepository for PgConnection {
    async fn add_thread_anchor(
        &mut self,
        workspace_id: Uuid,
        new_anchor: NewWorkspaceThreadAnchor,
        actor: Uuid,
    ) -> Result<WorkspaceThreadAnchor> {
        self.transaction(async |conn| {
            use schema::{workspace_thread_anchors, workspace_thread_events};

            let anchor = diesel::insert_into(workspace_thread_anchors::table)
                .values(&new_anchor)
                .returning(WorkspaceThreadAnchor::as_returning())
                .get_result::<WorkspaceThreadAnchor>(conn)
                .await
                .map_err(Error::from)?;

            // The event snapshots the anchor so the timeline renders it even after
            // the anchor row is removed.
            diesel::insert_into(workspace_thread_events::table)
                .values(&NewWorkspaceThreadEvent {
                    workspace_id,
                    thread_id: anchor.thread_id,
                    kind: ThreadEventKind::AnchorAdded,
                    actor_account_id: Some(actor),
                    target: Some(anchor_snapshot(&anchor)),
                })
                .execute(conn)
                .await
                .map_err(Error::from)?;

            Ok(anchor)
        })
        .await
    }

    async fn remove_thread_anchor(
        &mut self,
        workspace_id: Uuid,
        anchor_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceThreadAnchor> {
        self.transaction(async |conn| {
            use schema::workspace_thread_anchors::{self, dsl};
            use schema::workspace_thread_events;

            let anchor = diesel::update(
                workspace_thread_anchors::table
                    .filter(dsl::id.eq(anchor_id))
                    .filter(dsl::deleted_at.is_null()),
            )
            .set(dsl::deleted_at.eq(now))
            .returning(WorkspaceThreadAnchor::as_returning())
            .get_result::<WorkspaceThreadAnchor>(conn)
            .await
            .map_err(Error::from)?;

            diesel::insert_into(workspace_thread_events::table)
                .values(&NewWorkspaceThreadEvent {
                    workspace_id,
                    thread_id: anchor.thread_id,
                    kind: ThreadEventKind::AnchorRemoved,
                    actor_account_id: Some(actor),
                    target: Some(anchor_snapshot(&anchor)),
                })
                .execute(conn)
                .await
                .map_err(Error::from)?;

            Ok(anchor)
        })
        .await
    }

    async fn find_thread_anchor(
        &mut self,
        thread_id: Uuid,
        anchor_id: Uuid,
    ) -> Result<Option<WorkspaceThreadAnchor>> {
        use schema::workspace_thread_anchors::{self, dsl};

        workspace_thread_anchors::table
            .filter(dsl::id.eq(anchor_id))
            .filter(dsl::thread_id.eq(thread_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceThreadAnchor::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_thread_anchors(&mut self, thread_id: Uuid) -> Result<Vec<WorkspaceThreadAnchor>> {
        use schema::workspace_thread_anchors::{self, dsl};

        workspace_thread_anchors::table
            .filter(dsl::thread_id.eq(thread_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceThreadAnchor::as_select())
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn list_anchors_for_threads(
        &mut self,
        thread_ids: &[Uuid],
    ) -> Result<Vec<WorkspaceThreadAnchor>> {
        use schema::workspace_thread_anchors::{self, dsl};

        if thread_ids.is_empty() {
            return Ok(Vec::new());
        }

        // One query for the whole page's anchors. Ordered by thread then creation
        // so the caller can group into per-thread runs, each already oldest-first.
        workspace_thread_anchors::table
            .filter(dsl::thread_id.eq_any(thread_ids))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceThreadAnchor::as_select())
            .order((dsl::thread_id.asc(), dsl::created_at.asc(), dsl::id.asc()))
            .load(self)
            .await
            .map_err(Error::from)
    }
}
