//! Workspace thread-event repository: the immutable timeline entries (opened,
//! closed, reopened, renamed, anchor added/removed) that the reader interleaves
//! with the comments. Also houses the shared helpers the thread and anchor
//! repositories use to record those events.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceThreadEvent, WorkspaceThread, WorkspaceThreadAnchor, WorkspaceThreadEvent,
};
use crate::types::{AccountRefRow, ThreadEventKind};
use crate::{Error, PgConnection, Result, schema};

/// Read operations on a thread's timeline events.
pub trait WorkspaceThreadEventRepository {
    /// Lists a thread's timeline events, oldest first, each paired with the
    /// actor's account reference when the actor still exists.
    fn list_thread_events(
        &mut self,
        thread_id: Uuid,
    ) -> impl Future<Output = Result<Vec<(WorkspaceThreadEvent, Option<AccountRefRow>)>>> + Send;
}

impl WorkspaceThreadEventRepository for PgConnection {
    async fn list_thread_events(
        &mut self,
        thread_id: Uuid,
    ) -> Result<Vec<(WorkspaceThreadEvent, Option<AccountRefRow>)>> {
        use schema::accounts;
        use schema::workspace_thread_events::{self, dsl};

        // The actor is nullable (SET NULL on account removal), so left-join it and
        // load the account-ref column group as an `Option`.
        workspace_thread_events::table
            .left_join(accounts::table.on(dsl::actor_account_id.eq(accounts::id.nullable())))
            .filter(dsl::thread_id.eq(thread_id))
            .select((
                WorkspaceThreadEvent::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                )
                    .nullable(),
            ))
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .load(self)
            .await
            .map_err(Error::from)
    }
}

/// Inserts one thread timeline event. Shared by the thread and anchor
/// repositories, which record events as part of their own transactions.
pub(crate) async fn record_event(
    conn: &mut PgConnection,
    thread: &WorkspaceThread,
    kind: ThreadEventKind,
    actor: Uuid,
    target: Option<Value>,
) -> Result<()> {
    use schema::workspace_thread_events;

    diesel::insert_into(workspace_thread_events::table)
        .values(&NewWorkspaceThreadEvent {
            workspace_id: thread.workspace_id,
            thread_id: thread.id,
            kind,
            actor_account_id: Some(actor),
            target,
        })
        .execute(conn)
        .await
        .map_err(Error::from)?;

    Ok(())
}

/// A JSON snapshot of an anchor for a timeline event's `target`, so the timeline
/// renders a removed anchor without its (now soft-deleted) row.
pub(crate) fn anchor_snapshot(anchor: &WorkspaceThreadAnchor) -> Value {
    serde_json::json!({ "anchorId": anchor.id, "anchor": anchor.anchor })
}
