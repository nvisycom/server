//! Workspace assignments repository for distributing file review work.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspaceAssignment, UpdateWorkspaceAssignment, WorkspaceAssignment};
use crate::types::{
    AccountRefRow, AssignmentFilter, ConstraintViolation, CursorPage, CursorPagination,
    WorkspaceAssignmentConstraints, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating a workspace's assignments: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentCursor {
    /// When the assignment was created.
    pub created_at: Timestamp,
    /// Assignment id (tiebreaker).
    pub id: uuid::Uuid,
}

/// One assignment paired with the reviewer's account reference and the name of
/// the file under review.
///
/// The file is LEFT-joined, so an assignment whose file was removed by retention
/// yields a `None` name rather than dropping the row.
#[derive(Debug, Clone, PartialEq)]
pub struct AssignmentListRow {
    /// The assignment row.
    pub assignment: WorkspaceAssignment,
    /// The reviewer the file is assigned to.
    pub assignee: AccountRefRow,
    /// Display name of the file under review, when the file still exists.
    pub file_name: Option<String>,
}

/// The result of a
/// [`create_workspace_assignment`](WorkspaceAssignmentRepository::create_workspace_assignment)
/// call.
#[derive(Debug, Clone, PartialEq)]
pub enum CreateAssignmentOutcome {
    /// The assignment was created by this call.
    Created(WorkspaceAssignment),
    /// The reviewer is already assigned this file; the call is a no-op.
    AlreadyAssigned,
}

/// Repository for workspace assignment database operations.
///
/// An assignment is one reviewer's assignment of one file; a file may have
/// several (like GitHub assignees). Assignments live in their own table rather
/// than as columns on the file so a file can carry many at once.
pub trait WorkspaceAssignmentRepository {
    /// Assigns a file to a reviewer.
    ///
    /// A reviewer is assigned a given file at most once: a repeat is reported as
    /// [`AlreadyAssigned`](CreateAssignmentOutcome::AlreadyAssigned) rather than
    /// inserting a second row.
    fn create_workspace_assignment(
        &mut self,
        new_assignment: NewWorkspaceAssignment,
    ) -> impl Future<Output = Result<CreateAssignmentOutcome>> + Send;

    /// Finds an assignment by id within a specific workspace.
    fn find_assignment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        assignment_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceAssignment>>> + Send;

    /// Finds a reviewer's assignment on a file, if any — the `(file, assignee)`
    /// unique row.
    fn find_file_assignment_for_assignee(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
        assignee_account_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceAssignment>>> + Send;

    /// Lists a file's assignments, each paired with the reviewer's account
    /// reference ("who is assigned to this file").
    fn list_file_assignments(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<Vec<AssignmentListRow>>> + Send;

    /// Lists a workspace's assignments with cursor pagination, each paired with
    /// the reviewer's account reference and the file name.
    fn cursor_list_workspace_assignments(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<AssignmentCursor>,
        filter: &AssignmentFilter,
    ) -> impl Future<Output = Result<CursorPage<AssignmentListRow>>> + Send;

    /// Updates an assignment's review status.
    fn update_workspace_assignment(
        &mut self,
        assignment_id: Uuid,
        updates: UpdateWorkspaceAssignment,
    ) -> impl Future<Output = Result<WorkspaceAssignment>> + Send;

    /// Deletes an assignment (unassigns the reviewer).
    fn delete_workspace_assignment(
        &mut self,
        assignment_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceAssignmentRepository for PgConnection {
    async fn create_workspace_assignment(
        &mut self,
        new_assignment: NewWorkspaceAssignment,
    ) -> Result<CreateAssignmentOutcome> {
        use schema::workspace_assignments;

        let insert = diesel::insert_into(workspace_assignments::table)
            .values(&new_assignment)
            .returning(WorkspaceAssignment::as_returning())
            .get_result(self)
            .await;

        match insert {
            Ok(assignment) => Ok(CreateAssignmentOutcome::Created(assignment)),
            Err(err) => {
                let err = Error::from(err);
                if matches!(
                    err.constraint_violation(),
                    Some(ConstraintViolation::WorkspaceAssignment(
                        WorkspaceAssignmentConstraints::FileAssigneeUnique
                    ))
                ) {
                    Ok(CreateAssignmentOutcome::AlreadyAssigned)
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn find_assignment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        assignment_id: Uuid,
    ) -> Result<Option<WorkspaceAssignment>> {
        use schema::workspace_assignments::{self, dsl};

        workspace_assignments::table
            .filter(dsl::id.eq(assignment_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .select(WorkspaceAssignment::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn find_file_assignment_for_assignee(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
        assignee_account_id: Uuid,
    ) -> Result<Option<WorkspaceAssignment>> {
        use schema::workspace_assignments::{self, dsl};

        // Matches the `(file_id, assignee_account_id)` unique key; the workspace
        // filter keeps the lookup scoped even though the pair is already unique.
        workspace_assignments::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::assignee_account_id.eq(assignee_account_id))
            .select(WorkspaceAssignment::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_file_assignments(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Vec<AssignmentListRow>> {
        use schema::workspace_assignments::dsl;
        use schema::{accounts, workspace_assignments, workspace_files};

        // The assignee is one of two account FKs on the row, so the join names the
        // column explicitly rather than relying on an inferred `joinable!`.
        let rows: Vec<(WorkspaceAssignment, AccountRefRow, Option<String>)> =
            workspace_assignments::table
                .inner_join(accounts::table.on(dsl::assignee_account_id.eq(accounts::id)))
                .left_join(workspace_files::table.on(dsl::file_id.eq(workspace_files::id)))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::file_id.eq(file_id))
                .select((
                    WorkspaceAssignment::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                    workspace_files::display_name.nullable(),
                ))
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .load(self)
                .await
                .map_err(Error::from)?;

        Ok(rows
            .into_iter()
            .map(|(assignment, assignee, file_name)| AssignmentListRow {
                assignment,
                assignee,
                file_name,
            })
            .collect())
    }

    async fn cursor_list_workspace_assignments(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<AssignmentCursor>,
        filter: &AssignmentFilter,
    ) -> Result<CursorPage<AssignmentListRow>> {
        use schema::workspace_assignments::dsl;
        use schema::{accounts, workspace_assignments, workspace_files};

        // One scoped builder for both the count and the page, so a future filter
        // cannot be added to one and forgotten on the other. The assignee is one
        // of two account FKs, so the join names it explicitly; the file is
        // LEFT-joined so a removed file yields a null name rather than dropping
        // the row.
        let scoped = || {
            let mut query = workspace_assignments::table
                .inner_join(accounts::table.on(dsl::assignee_account_id.eq(accounts::id)))
                .left_join(workspace_files::table.on(dsl::file_id.eq(workspace_files::id)))
                .filter(dsl::workspace_id.eq(workspace_id))
                .into_boxed();
            if let Some(assignee_account_id) = filter.assignee_account_id {
                query = query.filter(dsl::assignee_account_id.eq(assignee_account_id));
            }
            if let Some(status) = filter.status {
                query = query.filter(dsl::status.eq(status));
            }
            if let Some(file_id) = filter.file_id {
                query = query.filter(dsl::file_id.eq(file_id));
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let selection = (
            WorkspaceAssignment::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
            workspace_files::display_name.nullable(),
        );

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspaceAssignment, AccountRefRow, Option<String>)> = keyset!(
            scoped(),
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select(selection)
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        let items = rows
            .into_iter()
            .map(|(assignment, assignee, file_name)| AssignmentListRow {
                assignment,
                assignee,
                file_name,
            })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            AssignmentCursor {
                created_at: row.assignment.created_at.into(),
                id: row.assignment.id,
            }
        }))
    }

    async fn update_workspace_assignment(
        &mut self,
        assignment_id: Uuid,
        updates: UpdateWorkspaceAssignment,
    ) -> Result<WorkspaceAssignment> {
        use schema::workspace_assignments::{self, dsl};

        // An all-`None` changeset would make Diesel emit an empty `SET`, which
        // Postgres rejects as a syntax error. Reject it up front so a caller with
        // nothing to change gets a clear error rather than a raw SQL failure.
        if updates.status.is_none() {
            return Err(Error::unexpected(
                "update_workspace_assignment called with no fields to update",
            ));
        }

        diesel::update(workspace_assignments::table.filter(dsl::id.eq(assignment_id)))
            .set(&updates)
            .returning(WorkspaceAssignment::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn delete_workspace_assignment(&mut self, assignment_id: Uuid) -> Result<()> {
        use schema::workspace_assignments::{self, dsl};

        diesel::delete(workspace_assignments::table.filter(dsl::id.eq(assignment_id)))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CreateAssignmentOutcome, WorkspaceAssignmentRepository};
    use crate::model::{
        NewAccount, NewWorkspaceAssignment, NewWorkspaceFile, UpdateWorkspaceAssignment,
    };
    use crate::query::{AccountRepository, WorkspaceFileRepository};
    use crate::test_util::TestDatabase;
    use crate::types::{AssignmentFilter, AssignmentStatus, CursorPagination};

    #[tokio::test]
    async fn create_is_idempotent_per_file_and_reviewer() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_file().await;
        let reviewer = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let created = conn
            .create_workspace_assignment(NewWorkspaceAssignment::test(
                seeded.workspace_id,
                seeded.file_id,
                reviewer,
            ))
            .await?;
        assert!(matches!(created, CreateAssignmentOutcome::Created(_)));

        // Assigning the same reviewer the same file again is a reported no-op, not
        // a second row.
        let again = conn
            .create_workspace_assignment(NewWorkspaceAssignment::test(
                seeded.workspace_id,
                seeded.file_id,
                reviewer,
            ))
            .await?;
        assert_eq!(again, CreateAssignmentOutcome::AlreadyAssigned);

        let file_rows = conn
            .list_file_assignments(seeded.workspace_id, seeded.file_id)
            .await?;
        assert_eq!(file_rows.len(), 1);
        assert_eq!(file_rows[0].assignment.assignee_account_id, reviewer);

        // The targeted (file, assignee) lookup finds the same row, and returns
        // None for a reviewer who has no assignment on the file.
        let found = conn
            .find_file_assignment_for_assignee(seeded.workspace_id, seeded.file_id, reviewer)
            .await?;
        assert_eq!(found.map(|a| a.assignee_account_id), Some(reviewer));
        let other = db.seed_account().await;
        assert!(
            conn.find_file_assignment_for_assignee(seeded.workspace_id, seeded.file_id, other)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn status_update_and_delete_round_trip() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_file().await;
        let reviewer = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let CreateAssignmentOutcome::Created(assignment) = conn
            .create_workspace_assignment(NewWorkspaceAssignment::test(
                seeded.workspace_id,
                seeded.file_id,
                reviewer,
            ))
            .await?
        else {
            panic!("expected a fresh assignment");
        };
        assert_eq!(assignment.status, AssignmentStatus::Assigned);

        let updated = conn
            .update_workspace_assignment(
                assignment.id,
                UpdateWorkspaceAssignment {
                    status: Some(AssignmentStatus::Done),
                },
            )
            .await?;
        assert_eq!(updated.status, AssignmentStatus::Done);

        conn.delete_workspace_assignment(assignment.id).await?;
        assert!(
            conn.find_assignment_in_workspace(seeded.workspace_id, assignment.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_filters_by_assignee_and_status() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_file().await;
        let alice = db.seed_account().await;
        let bob = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        for reviewer in [alice, bob] {
            let _ = conn
                .create_workspace_assignment(NewWorkspaceAssignment::test(
                    seeded.workspace_id,
                    seeded.file_id,
                    reviewer,
                ))
                .await?;
        }

        // No filter: both reviewers' assignments.
        let all = conn
            .cursor_list_workspace_assignments(
                seeded.workspace_id,
                CursorPagination::new(50),
                &AssignmentFilter::default(),
            )
            .await?;
        assert_eq!(all.items.len(), 2);

        // Filter to one reviewer.
        let just_alice = conn
            .cursor_list_workspace_assignments(
                seeded.workspace_id,
                CursorPagination::new(50),
                &AssignmentFilter {
                    assignee_account_id: Some(alice),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(just_alice.items.len(), 1);
        assert_eq!(just_alice.items[0].assignment.assignee_account_id, alice);

        // A status no assignment holds returns nothing.
        let none = conn
            .cursor_list_workspace_assignments(
                seeded.workspace_id,
                CursorPagination::new(50),
                &AssignmentFilter {
                    status: Some(AssignmentStatus::Done),
                    ..Default::default()
                },
            )
            .await?;
        assert!(none.items.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn assigner_attribution_round_trips() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_file().await;
        let reviewer = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        // `assigned_account_id` records who made the assignment, distinct from the
        // assignee it is made to.
        let CreateAssignmentOutcome::Created(assignment) = conn
            .create_workspace_assignment(NewWorkspaceAssignment {
                assigned_account_id: Some(seeded.account_id),
                ..NewWorkspaceAssignment::test(seeded.workspace_id, seeded.file_id, reviewer)
            })
            .await?
        else {
            panic!("expected a fresh assignment");
        };
        assert_eq!(assignment.assigned_account_id, Some(seeded.account_id));
        assert_eq!(assignment.assignee_account_id, reviewer);
        Ok(())
    }

    #[tokio::test]
    async fn list_file_assignments_joins_assignee_and_file_name() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A named reviewer and a named file, so the joins have distinct values to
        // return.
        let reviewer = conn.create_account(NewAccount::test()).await?;
        let file = conn
            .create_workspace_file(NewWorkspaceFile {
                display_name: Some("quarterly-report.pdf".to_owned()),
                ..NewWorkspaceFile::test(seeded.workspace_id, seeded.account_id)
            })
            .await?;

        let _ = conn
            .create_workspace_assignment(NewWorkspaceAssignment::test(
                seeded.workspace_id,
                file.id,
                reviewer.id,
            ))
            .await?;

        let rows = conn
            .list_file_assignments(seeded.workspace_id, file.id)
            .await?;
        assert_eq!(rows.len(), 1);
        // The assignee join names the reviewer, not the assigner.
        assert_eq!(rows[0].assignee.username, reviewer.username);
        // The file join names the file under review.
        assert_eq!(rows[0].file_name.as_deref(), Some("quarterly-report.pdf"));
        Ok(())
    }
}
