//! Project member management handlers.
//!
//! This module provides comprehensive project member management functionality,
//! allowing project administrators to view, add, modify, and remove project
//! members. All operations are secured with proper authorization and follow
//! role-based access control principles.

use aide::axum::ApiRouter;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::UpdateProjectMember;
use nvisy_postgres::query::{
    Pagination as PgPagination, ProjectMemberRepository, ProjectRepository,
};
use nvisy_postgres::types::{ProjectRole, ProjectVisibility};

use crate::extract::{AuthProvider, AuthState, Path, Permission, ValidateJson};
use crate::handler::request::{MemberPathParams, Pagination, ProjectPathParams, UpdateMemberRole};
use crate::handler::response::{Member, Members};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project member operations.
const TRACING_TARGET: &str = "nvisy_server::handler::members";

/// Lists all members of a project.
///
/// Returns a paginated list of project members with their roles and status.
/// Requires `ViewMembers` permission. Returns an empty list for private projects.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn list_members(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Members>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing project members");

    auth_state
        .authorize_project(&pg_client, path_params.project_id, Permission::ViewMembers)
        .await?;

    let Some(project) = pg_client.find_project_by_id(path_params.project_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("project")
            .with_message("Project not found"));
    };

    // Return empty list for private projects
    let members: Members = if project.visibility == ProjectVisibility::Private {
        tracing::debug!(target: TRACING_TARGET, "Project is private, returning empty list");
        Vec::new()
    } else {
        let project_members = pg_client
            .list_project_members(path_params.project_id, pagination.into())
            .await?;

        project_members.into_iter().map(Member::from).collect()
    };

    tracing::info!(
        target: TRACING_TARGET,
        member_count = members.len(),
        "Project members listed successfully",
    );

    Ok((StatusCode::OK, Json(members)))
}

/// Gets detailed information about a specific project member.
///
/// Returns comprehensive information about a project member, including their role,
/// permissions, and activity status. Requires `ViewMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        member_id = %path_params.account_id,
    )
)]
async fn get_member(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::debug!(target: TRACING_TARGET, "Retrieving project member details");

    auth_state
        .authorize_project(&pg_client, path_params.project_id, Permission::ViewMembers)
        .await?;

    let Some(project) = pg_client.find_project_by_id(path_params.project_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("project")
            .with_message("Project not found"));
    };

    if project.visibility.is_private() {
        return Err(ErrorKind::Forbidden
            .with_resource("project")
            .with_message("Cannot view members of a private project"));
    }

    let Some(project_member) = pg_client
        .find_project_member(path_params.project_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("project_member")
            .with_message("Project member not found"));
    };

    tracing::debug!(
        target: TRACING_TARGET,
        member_role = ?project_member.member_role,
        "Project member retrieved successfully",
    );

    Ok((StatusCode::OK, Json(project_member.into())))
}

/// Removes a member from a project.
///
/// Permanently removes a member from the project. This action cannot be undone.
/// The member will lose all access to the project and its resources.
/// Requires `RemoveMembers` permission. Cannot remove the last admin.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        member_id = %path_params.account_id,
    )
)]
async fn delete_member(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Removing project member");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::RemoveMembers,
        )
        .await?;

    // Prevent self-removal (use leave endpoint instead)
    if auth_state.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot remove yourself. Use the leave project endpoint instead."));
    }

    let Some(member_to_remove) = pg_client
        .find_project_member(path_params.project_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("project_member"));
    };

    // Prevent removing the last admin
    if member_to_remove.member_role == ProjectRole::Admin {
        let admin_count = count_active_admins(&pg_client, path_params.project_id).await?;

        if admin_count <= 1 {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot remove last admin")
                .with_context("Promote another member to admin first"));
        }
    }

    pg_client
        .remove_project_member(path_params.project_id, path_params.account_id)
        .await?;

    tracing::warn!(target: TRACING_TARGET, "Project member removed successfully");

    Ok(StatusCode::OK)
}

/// Updates a project member's role.
///
/// Allows project admins to change a member's permission level.
/// Cannot update your own role. Cannot demote the last admin.
/// Requires `ManageRoles` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        member_id = %path_params.account_id,
        new_role = ?request.role,
    )
)]
async fn update_member(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<MemberPathParams>,
    ValidateJson(request): ValidateJson<UpdateMemberRole>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::info!(target: TRACING_TARGET, "Updating project member role");

    auth_state
        .authorize_project(&pg_client, path_params.project_id, Permission::ManageRoles)
        .await?;

    // Prevent self-role-update
    if auth_state.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot update your own role")
            .with_context("Ask another admin to update your role"));
    }

    let Some(current_member) = pg_client
        .find_project_member(path_params.project_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("project_member"));
    };

    // Prevent demoting the last admin
    if current_member.member_role == ProjectRole::Admin && request.role != ProjectRole::Admin {
        let admin_count = count_active_admins(&pg_client, path_params.project_id).await?;

        if admin_count <= 1 {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot demote last admin")
                .with_context("Promote another member to admin first"));
        }
    }

    let changes = UpdateProjectMember {
        member_role: Some(request.role),
        ..Default::default()
    };

    let updated_member = pg_client
        .update_project_member(path_params.project_id, path_params.account_id, changes)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        new_role = ?updated_member.member_role,
        "Member role updated successfully",
    );

    Ok((StatusCode::OK, Json(updated_member.into())))
}

/// Counts the number of active admins in a project.
async fn count_active_admins(
    pg_client: &PgClient,
    project_id: uuid::Uuid,
) -> Result<usize, nvisy_postgres::PgError> {
    let all_members = pg_client
        .list_project_members(
            project_id,
            PgPagination {
                limit: 1000,
                offset: 0,
            },
        )
        .await?;

    Ok(all_members
        .iter()
        .filter(|m| m.member_role == ProjectRole::Admin && m.is_active)
        .count())
}

/// Returns a [`Router`] with all project member related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/projects/:project_id/members/", get(list_members))
        .api_route(
            "/projects/:project_id/members/:account_id/",
            get(get_member),
        )
        .api_route(
            "/projects/:project_id/members/:account_id/",
            delete(delete_member),
        )
        .api_route(
            "/projects/:project_id/members/:account_id/role",
            patch(update_member),
        )
}
