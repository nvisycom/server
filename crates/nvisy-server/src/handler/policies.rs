//! Workspace policy management handlers.
//!
//! Policies are structured redaction governance documents (the engine's
//! Policy type) consumed by the redaction pipeline. The definition is
//! validated against the schema, then stored encrypted (XChaCha20-Poly1305,
//! workspace-derived key) as a BYTEA column in PostgreSQL, scoped to a
//! workspace.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy};
use nvisy_postgres::query::WorkspacePolicyRepository;
use nvisy_postgres::types::Username;
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, WorkspaceContext,
};
use crate::handler::request::{CreatePolicy, CursorPagination, PolicyPathParams, UpdatePolicy};
use crate::handler::response::{ErrorResponse, PoliciesPage, Policy};
use crate::handler::{Error, Result};
use crate::service::{CryptoService, ServiceState};

/// Tracing target for workspace policy operations.
const TRACING_TARGET: &str = "nvisy_server::handler::policies";

/// Creates a new workspace policy.
///
/// The request body carries a structured policy definition; its name,
/// description, and version drive the stored record unless overridden.
/// Requires `ManagePolicies` permission for the workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn create_policy(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    ValidateJson(request): ValidateJson<CreatePolicy>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace policy");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManagePolicies)
        .await?;

    let definition = &request.definition;
    let name = request.name.unwrap_or_else(|| definition.name.to_string());
    let description = request
        .description
        .or_else(|| definition.description.clone());
    let version = definition.version.to_string();
    let encrypted = crypto.encrypt_json(workspace.id, definition)?;

    let new_policy = NewWorkspacePolicy {
        workspace_id: workspace.id,
        account_id: auth_state.account_id,
        slug: request.slug,
        name,
        description,
        version,
        definition: encrypted,
        metadata: None,
    };

    let policy = conn.create_workspace_policy(new_policy).await?;

    tracing::info!(target: TRACING_TARGET, policy_slug = %policy.slug, "Policy created");

    let (policy, creator_username) =
        find_policy(&mut conn, workspace.id, policy.slug.as_str()).await?;

    Ok((
        StatusCode::CREATED,
        Json(Policy::from_model(
            policy,
            workspace.slug,
            creator_username,
            &crypto,
        )?),
    ))
}

fn create_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create policy")
        .description("Creates a structured redaction policy for the workspace.")
        .response::<201, Json<Policy>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all policies for a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_policies(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<PoliciesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace policies");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPolicies)
        .await?;

    let page = conn
        .cursor_list_workspace_policies(workspace.id, pagination.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        policy_count = page.items.len(),
        "Workspace policies listed",
    );

    let page = PoliciesPage::try_from_cursor_page(page, |(model, creator_username)| {
        Policy::from_model(model, workspace.slug.clone(), creator_username, &crypto)
    })?;

    Ok((StatusCode::OK, Json(page)))
}

fn list_policies_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List policies")
        .description("Returns all policies for the workspace.")
        .response::<200, Json<PoliciesPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace policy.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn read_policy(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PolicyPathParams>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace policy");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewPolicies)
        .await?;

    let (policy, creator_username) =
        find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace policy read");

    Ok((
        StatusCode::OK,
        Json(Policy::from_model(
            policy,
            workspace.slug,
            creator_username,
            &crypto,
        )?),
    ))
}

fn read_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get policy")
        .description("Returns a single policy.")
        .response::<200, Json<Policy>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace policy.
///
/// All fields are optional; replacing the definition replaces the whole
/// policy body (and its version). Requires `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn update_policy(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PolicyPathParams>,
    ValidateJson(request): ValidateJson<UpdatePolicy>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace policy");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManagePolicies)
        .await?;

    // Confirm the policy exists in this workspace before mutating.
    let (existing, _) = find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;

    let (version, definition) = match &request.definition {
        Some(definition) => {
            let encrypted = crypto.encrypt_json(workspace.id, definition)?;
            (Some(definition.version.to_string()), Some(encrypted))
        }
        None => (None, None),
    };

    let updates = UpdateWorkspacePolicy {
        name: request.name,
        description: request.description,
        version,
        definition,
        ..Default::default()
    };

    conn.update_workspace_policy(existing.id, updates).await?;

    let (policy, creator_username) =
        find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;

    tracing::info!(target: TRACING_TARGET, "Policy updated");

    Ok((
        StatusCode::OK,
        Json(Policy::from_model(
            policy,
            workspace.slug,
            creator_username,
            &crypto,
        )?),
    ))
}

fn update_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update policy")
        .description("Updates policy fields. Replacing the definition replaces the whole body.")
        .response::<200, Json<Policy>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace policy.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn delete_policy(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<PolicyPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace policy");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManagePolicies)
        .await?;

    // Confirm the policy exists in this workspace before deleting.
    let (existing, _) = find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;

    conn.delete_workspace_policy(existing.id).await?;

    tracing::info!(target: TRACING_TARGET, "Policy deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete policy")
        .description("Soft-deletes the policy from the workspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a policy within a workspace by slug, with its creator's handle, or
/// returns a NotFound error.
async fn find_policy(
    conn: &mut PgConn,
    workspace_id: Uuid,
    policy_slug: &str,
) -> Result<(WorkspacePolicy, Username)> {
    conn.find_policy_in_workspace_by_slug(workspace_id, policy_slug)
        .await?
        .ok_or_else(|| Error::not_found("policy"))
}

/// Returns routes for workspace policy management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/policies/",
            post_with(create_policy, create_policy_docs)
                .get_with(list_policies, list_policies_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/policies/{policySlug}/",
            get_with(read_policy, read_policy_docs)
                .put_with(update_policy, update_policy_docs)
                .delete_with(delete_policy, delete_policy_docs),
        )
        .with_path_items(|item| item.tag("Policies"))
}
