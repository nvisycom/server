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
use elide_pipeline::policy::PolicyDefinition;
use nvisy_postgres::model::{NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy};
use nvisy_postgres::query::WorkspacePolicyRepository;
use nvisy_postgres::types::WithAccountRef;
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, SecurityContext, ValidateJson,
    WorkspaceContext,
};
use crate::handler::request::{CreatePolicy, CursorPagination, PolicyPathParams, UpdatePolicy};
use crate::handler::response::{ErrorResponse, PoliciesPage, Policy, PolicySummary};
use crate::handler::utility::resolve_account_ref;
use crate::handler::{Error, Result};
use crate::service::{
    CryptoService, EventEmitter, EventOrigin, PolicyRef, ServiceState, WorkspaceEvent,
};

/// Tracing target for workspace policy operations.
const TRACING_TARGET: &str = "nvisy_server::handler::policies";

/// Creates a new workspace policy.
///
/// The request body carries a structured policy definition; its name and
/// description drive the stored record unless overridden. Requires
/// `ManagePolicies` permission for the workspace.
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
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreatePolicy>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace policy");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManagePolicies)
        .await?;

    // Resolve the body (inline or a built-in template). `into_definition` mints a
    // fresh id and stamps the template origin (server-owned).
    let definition = request.body.into_definition();

    let display_name = request
        .display_name
        .unwrap_or_else(|| definition.name.to_string());
    let description = request
        .description
        .or_else(|| definition.description.clone().map(Into::into));
    let encrypted = crypto.encrypt_json(workspace.id, &definition)?;

    let new_policy = NewWorkspacePolicy {
        workspace_id: workspace.id,
        account_id: auth_state.account_id,
        slug: request.slug,
        display_name,
        description,
        definition: encrypted,
        metadata: None,
    };

    // Insert the policy and record the outbox event atomically, so the event is
    // never lost, nor recorded for an insert that rolled back.
    let policy = conn
        .transaction(async |conn| {
            let policy = conn.create_workspace_policy(new_policy).await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id: auth_state.account_id,
                    security: &security,
                },
                WorkspaceEvent::PolicyCreated(PolicyRef {
                    policy_id: policy.id,
                    policy_slug: policy.slug.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(policy)
        })
        .await?;

    tracing::info!(target: TRACING_TARGET, policy_slug = %policy.slug, "Policy created");

    // The creator is the authenticated caller; resolve their handle directly.
    let creator = resolve_account_ref(&mut conn, auth_state.account_id).await?;

    let response = Policy::from_model(policy, workspace.slug, creator, &crypto)?;

    Ok((StatusCode::CREATED, Json(response)))
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

    // The list carries only metadata; the encrypted definition is decrypted only
    // by the single-policy endpoint, so a page costs no per-item decryption.
    let page = PoliciesPage::from_cursor_page(page, |wc| {
        PolicySummary::from_model(wc.item, workspace.slug.clone(), wc.account.into())
    });

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

    let found = find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace policy read");

    Ok((
        StatusCode::OK,
        Json(Policy::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
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
/// policy body. Requires `ManagePolicies` permission.
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
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdatePolicy>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace policy");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManagePolicies)
        .await?;

    // Confirm the policy exists in this workspace before mutating.
    let existing = find_policy(&mut conn, workspace.id, &path_params.policy_slug)
        .await?
        .item;

    // A replaced body keeps the policy's server-owned template origin: the caller
    // authored new rules, but where the policy came from is provenance the client
    // cannot set or clear. Carry the stored origin forward onto the new draft.
    let definition = match request.definition {
        Some(draft) => {
            let template = crypto
                .decrypt_json::<PolicyDefinition>(workspace.id, &existing.definition)?
                .template;
            let definition = draft.into_definition(template);
            Some(crypto.encrypt_json(workspace.id, &definition)?)
        }
        None => None,
    };

    let updates = UpdateWorkspacePolicy {
        display_name: request.display_name,
        description: request.description,
        definition,
        ..Default::default()
    };

    // Update the policy and record the outbox event atomically, so the event is
    // never lost, nor recorded for an update that rolled back.
    let policy_id = existing.id;
    let policy_slug = existing.slug.clone();
    conn.transaction(async |conn| {
        conn.update_workspace_policy(policy_id, updates).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_state.account_id,
                security: &security,
            },
            WorkspaceEvent::PolicyUpdated(PolicyRef {
                policy_id,
                policy_slug,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    let found = find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;

    let response = Policy::from_model(found.item, workspace.slug, found.account.into(), &crypto)?;

    tracing::info!(target: TRACING_TARGET, "Policy updated");

    Ok((StatusCode::OK, Json(response)))
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
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace policy");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManagePolicies)
        .await?;

    // Confirm the policy exists in this workspace before deleting.
    let existing = find_policy(&mut conn, workspace.id, &path_params.policy_slug)
        .await?
        .item;
    let policy_id = existing.id;
    let policy_slug = existing.slug.clone();

    // Delete the policy and record the outbox event atomically, so the event is
    // never lost, nor recorded for a delete that rolled back.
    conn.transaction(async |conn| {
        conn.delete_workspace_policy(policy_id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_state.account_id,
                security: &security,
            },
            WorkspaceEvent::PolicyDeleted(PolicyRef {
                policy_id,
                policy_slug,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

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

/// Finds a policy within a workspace by slug, with its creator, or returns a
/// NotFound error.
async fn find_policy(
    conn: &mut PgConn,
    workspace_id: Uuid,
    policy_slug: &str,
) -> Result<WithAccountRef<WorkspacePolicy>> {
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
                .patch_with(update_policy, update_policy_docs)
                .delete_with(delete_policy, delete_policy_docs),
        )
        .with_path_items(|item| item.tag("Policies"))
}
