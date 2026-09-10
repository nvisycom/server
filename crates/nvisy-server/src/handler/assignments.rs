//! Assignment handlers: assign a file to reviewers, track review status.
//!
//! An assignment is one reviewer's assignment of one file for redaction review.
//! A file may be assigned to several reviewers at once (like GitHub assignees);
//! each assignment is its own resource with its own review status. Creating and
//! removing assignments requires `AssignTasks`; a reviewer may change the status
//! of their own assignment, and anyone with `AssignTasks` may change any.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{
    NewWorkspaceAssignment, UpdateWorkspaceAssignment, WorkspaceAssignment,
};
use nvisy_postgres::query::{
    AccountRepository, CreateAssignmentOutcome, WorkspaceAssignmentRepository,
    WorkspaceFileRepository, WorkspaceMemberRepository,
};
use nvisy_postgres::types::Handle;
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    Authorized, Json, Path, Permission, Query, SecurityContext, ValidateJson, markers,
};
use crate::handler::request::{
    AssignmentPathParams, CreateAssignment, CursorPagination, UpdateAssignment,
    WorkspaceAssignmentsQuery, WorkspaceFilePathParams,
};
use crate::handler::response::{Assignment, AssignmentsPage};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{
    AssignmentRef, EventEmitter, EventOrigin, FileRef, ServiceState, WorkspaceEvent,
};

/// Tracing target for assignment operations.
const TRACING_TARGET: &str = "nvisy_server::handler::assignments";

/// The literal assignee filter that resolves to the caller's own account.
const ASSIGNEE_ME: &str = "me";

/// Assigns a file to a reviewer.
///
/// A file may be assigned to several reviewers at once; assigning the same
/// reviewer again is a no-op that returns the existing assignment. The assignee
/// must be a member of the workspace. Requires `AssignTasks`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn create_assignment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::AssignTasks>,
    Path(path_params): Path<WorkspaceFilePathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateAssignment>,
) -> Result<(StatusCode, Json<Assignment>)> {
    tracing::debug!(target: TRACING_TARGET, "Assigning file to reviewer");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // The file must exist in the workspace.
    let file = conn
        .find_file_in_workspace(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    // The assignee must be a member of the workspace: a file is reviewed by the
    // people in its workspace, not by arbitrary accounts.
    let assignee = resolve_workspace_member(&mut conn, workspace.id, &request.assignee).await?;

    let new_assignment = NewWorkspaceAssignment {
        workspace_id: workspace.id,
        file_id: file.id,
        assignee_account_id: assignee,
        assigned_account_id: Some(authz.account_id),
        status: None,
    };

    // Create the assignment and record it in one transaction so the row and its
    // event commit or roll back together. A repeat assignment is a benign no-op:
    // return the existing row, unchanged, with no second event.
    let (assignment, created) = conn
        .transaction(
            async |conn| match conn.create_workspace_assignment(new_assignment).await? {
                CreateAssignmentOutcome::Created(assignment) => {
                    emit_assignment_event(
                        conn,
                        workspace_origin(workspace.id, authz.account_id, &security),
                        WorkspaceEvent::FileAssigned {
                            assignment: assignment_ref(
                                &assignment,
                                &file.display_name,
                                &request.assignee,
                            ),
                            // No self-notification: the actor already knows they
                            // assigned themselves.
                            notify: notify_target(assignee, authz.account_id),
                        },
                    )
                    .await?;
                    Ok::<_, Error>((assignment, true))
                }
                CreateAssignmentOutcome::AlreadyAssigned => {
                    let existing = conn
                        .find_file_assignment_for_assignee(workspace.id, file.id, assignee)
                        .await?
                        .ok_or_else(|| Error::not_found("workspace_assignment"))?;
                    Ok((existing, false))
                }
            },
        )
        .await?;

    let assignee_ref = resolve_account_ref(&mut conn, assignment.assignee_account_id).await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    tracing::info!(target: TRACING_TARGET, assignment_id = %assignment.id, created, "File assigned");

    Ok((
        status,
        Json(Assignment::from_model(
            assignment,
            assignee_ref,
            Some(file.display_name),
        )),
    ))
}

fn create_assignment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Assign a file")
        .description(
            "Assigns a file to a workspace member for review. A file may have \
             several reviewers; assigning the same reviewer again returns the \
             existing assignment. Requires the AssignTasks permission.",
        )
        .response::<201, Json<Assignment>>()
        .response::<200, Json<Assignment>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists a file's reviewers (its assignments), most recent first.
///
/// Requires `ViewAssignments`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        file_id = %path_params.file_id,
    )
)]
async fn list_file_assignments(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewAssignments>,
    Path(path_params): Path<WorkspaceFilePathParams>,
) -> Result<(StatusCode, Json<Vec<Assignment>>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing file assignments");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // The file must exist in the workspace, so a missing file is a 404 rather than
    // an empty list.
    conn.find_file_in_workspace(workspace.id, path_params.file_id)
        .await?
        .ok_or_else(|| Error::not_found("file"))?;

    let rows = conn
        .list_file_assignments(workspace.id, path_params.file_id)
        .await?;

    let assignments = rows
        .into_iter()
        .map(|row| Assignment::from_model(row.assignment, row.assignee.into(), row.file_name))
        .collect();

    Ok((StatusCode::OK, Json(assignments)))
}

fn list_file_assignments_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List a file's reviewers")
        .description("Returns the assignments on a file, most recent first.")
        .response::<200, Json<Vec<Assignment>>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists a workspace's assignments with cursor pagination.
///
/// Filter by `assignee` (a member handle, or `me` for the caller), `status`, and
/// `fileId`. Requires `ViewAssignments`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_workspace_assignments(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewAssignments>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceAssignmentsQuery>,
) -> Result<(StatusCode, Json<AssignmentsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace assignments");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Resolve the assignee filter: `me` is the caller, any other value is a member
    // handle. An unknown handle is a 404, not a silently empty page.
    let assignee_account_id = match query.assignee.as_deref() {
        None => None,
        Some(ASSIGNEE_ME) => Some(authz.account_id),
        Some(handle) => {
            let handle = Handle::try_from(handle.to_owned())
                .map_err(|_| Error::not_found("workspace_member"))?;
            Some(resolve_workspace_member(&mut conn, workspace.id, &handle).await?)
        }
    };

    let filter = query.into_filter(assignee_account_id);
    let page = conn
        .cursor_list_workspace_assignments(workspace.id, pagination.into(), &filter)
        .await?;

    let response = AssignmentsPage::from_cursor_page(page, |row| {
        Assignment::from_model(row.assignment, row.assignee.into(), row.file_name)
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_workspace_assignments_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace assignments")
        .description(
            "Returns the workspace's assignments, most recent first, with optional \
             assignee (a member handle or `me`), status, and file filters.",
        )
        .response::<200, Json<AssignmentsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Changes an assignment's review status.
///
/// Allowed for the assignee (their own review status) or a member with
/// `AssignTasks`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        assignment_id = %path_params.assignment_id,
    )
)]
async fn update_assignment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewAssignments>,
    Path(path_params): Path<AssignmentPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateAssignment>,
) -> Result<(StatusCode, Json<Assignment>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating assignment status");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let assignment = conn
        .find_assignment_in_workspace(workspace.id, path_params.assignment_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_assignment"))?;

    // Split authorization: the assignee may set their own review status; anyone
    // else needs AssignTasks (Editor tier), which the reviewer floor of this route
    // (ViewAssignments) does not grant on its own.
    let is_assignee = authz.account_id == assignment.assignee_account_id;
    let may_assign = Permission::AssignTasks.is_permitted_by_role(authz.member.member_role);
    if !is_assignee && !may_assign {
        return Err(ErrorKind::Forbidden
            .with_message("Only the assignee or a member who can assign tasks may change this")
            .with_resource("workspace_assignment"));
    }

    // Resolve the file name and the assignee reference (its handle drives the
    // event, and the same reference is returned in the response) before the
    // update. The assignee is a non-null FK, so it always resolves for a live row.
    let file_name = conn
        .find_file_in_workspace(workspace.id, assignment.file_id)
        .await?
        .map(|f| f.display_name)
        .unwrap_or_default();
    let assignee_ref = resolve_account_ref(&mut conn, assignment.assignee_account_id).await?;
    let assignee_handle = assignee_ref.username.clone();

    let updated = conn
        .transaction(async |conn| {
            let updated = conn
                .update_workspace_assignment(
                    assignment.id,
                    UpdateWorkspaceAssignment {
                        status: Some(request.status),
                    },
                )
                .await?;
            emit_assignment_event(
                conn,
                workspace_origin(workspace.id, authz.account_id, &security),
                WorkspaceEvent::AssignmentStatusChanged(assignment_ref(
                    &updated,
                    &file_name,
                    &assignee_handle,
                )),
            )
            .await?;
            Ok::<_, Error>(updated)
        })
        .await?;

    tracing::info!(target: TRACING_TARGET, status = ?updated.status, "Assignment status changed");

    Ok((
        StatusCode::OK,
        Json(Assignment::from_model(
            updated,
            assignee_ref,
            Some(file_name),
        )),
    ))
}

fn update_assignment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Change assignment status")
        .description(
            "Changes an assignment's review status. Allowed for the assignee or a \
             member with the AssignTasks permission.",
        )
        .response::<200, Json<Assignment>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Unassigns a reviewer from a file (deletes the assignment).
///
/// Requires `AssignTasks`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        assignment_id = %path_params.assignment_id,
    )
)]
async fn delete_assignment(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::AssignTasks>,
    Path(path_params): Path<AssignmentPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Unassigning reviewer from file");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let assignment = conn
        .find_assignment_in_workspace(workspace.id, path_params.assignment_id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_assignment"))?;

    // Resolve the file name and assignee handle for the event before the delete.
    // The assignee is a non-null FK, so it always resolves for a live row.
    let file_name = conn
        .find_file_in_workspace(workspace.id, assignment.file_id)
        .await?
        .map(|f| f.display_name)
        .unwrap_or_default();
    let assignee_handle = resolve_account_ref(&mut conn, assignment.assignee_account_id)
        .await?
        .username;

    conn.transaction(async |conn| {
        conn.delete_workspace_assignment(assignment.id).await?;
        emit_assignment_event(
            conn,
            workspace_origin(workspace.id, authz.account_id, &security),
            WorkspaceEvent::FileUnassigned {
                assignment: assignment_ref(&assignment, &file_name, &assignee_handle),
                // No self-notification when the actor unassigned themselves.
                notify: notify_target(assignment.assignee_account_id, authz.account_id),
            },
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "Reviewer unassigned");

    Ok(StatusCode::OK)
}

fn delete_assignment_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Unassign a reviewer")
        .description("Removes an assignment, unassigning the reviewer. Requires AssignTasks.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Resolves a member handle to its account id within the workspace, rejecting a
/// handle that is not a member of the workspace.
async fn resolve_workspace_member(
    conn: &mut PgConn,
    workspace_id: Uuid,
    username: &Handle,
) -> Result<Uuid> {
    let account = conn
        .find_account_by_username(username)
        .await?
        .ok_or_else(|| Error::not_found("workspace_member"))?;
    // Presence of a membership row is the check: a non-member handle resolves to
    // an account but no membership, so it is rejected the same as an unknown one.
    conn.find_workspace_member(workspace_id, account.id)
        .await?
        .ok_or_else(|| Error::not_found("workspace_member"))?;
    Ok(account.id)
}

/// The in-app notification target for an assignment change: the reviewer,
/// unless they are the actor who made the change (no self-notification).
fn notify_target(reviewer: Uuid, actor: Uuid) -> Option<Uuid> {
    (reviewer != actor).then_some(reviewer)
}

/// Builds the event origin shared by every assignment event.
fn workspace_origin<'a>(
    workspace_id: Uuid,
    account_id: Uuid,
    security: &'a SecurityContext,
) -> EventOrigin<'a> {
    EventOrigin {
        workspace_id,
        account_id,
        security,
    }
}

/// Builds an [`AssignmentRef`] from an assignment row, its file name, and the
/// assignee's handle.
fn assignment_ref(
    assignment: &WorkspaceAssignment,
    file_name: &str,
    assignee_username: &Handle,
) -> AssignmentRef {
    AssignmentRef {
        assignment_id: assignment.id,
        file: FileRef {
            file_id: assignment.file_id,
            file_name: file_name.to_owned(),
        },
        assignee_username: assignee_username.clone(),
    }
}

/// Emits one assignment event onto the outbox.
async fn emit_assignment_event(
    conn: &mut PgConn,
    origin: EventOrigin<'_>,
    event: WorkspaceEvent,
) -> Result<()> {
    conn.emit_event(origin, event).await?;
    Ok(())
}

/// Returns an [`ApiRouter`] with all assignment routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/files/{fileId}/assignments/",
            post_with(create_assignment, create_assignment_docs)
                .get_with(list_file_assignments, list_file_assignments_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/assignments/",
            get_with(list_workspace_assignments, list_workspace_assignments_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/assignments/{assignmentId}/",
            patch_with(update_assignment, update_assignment_docs)
                .delete_with(delete_assignment, delete_assignment_docs),
        )
        .with_path_items(|item| item.tag("Assignments"))
}
