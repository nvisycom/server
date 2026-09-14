//! Workspace policy management handlers.
//!
//! Policies are structured redaction governance documents (the engine's Policy
//! type) consumed by the redaction pipeline. A policy describes what to redact
//! and how — plain config, carrying no sensitive content — so the definition is
//! stored as plaintext JSONB on its version row, scoped to a workspace.
//!
//! These handlers are thin: they authorize, parse the request, delegate the
//! policy rules to [`WorkspacePolicyService`], and map the result to a response. The
//! domain logic lives in that service.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspacePolicy, CursorPagination, UpdateWorkspacePolicy, WorkspacePoliciesQuery,
    WorkspacePolicyPathParams,
};
use crate::handler::response::{PoliciesPage, WorkspacePolicy, WorkspacePolicySummary};
use crate::handler::utility::resolve_account_ref;
use crate::response::{ErrorResponse, Result};
use crate::service::{ServiceState, event};

/// Tracing target for workspace policy operations.
const TRACING_TARGET: &str = "nvisy_server::handler::policies";

/// Creates a new workspace policy.
///
/// The request body carries a structured policy definition; its name and
/// description drive the stored record unless overridden. A labels body creates
/// (or reuses) a one-shot policy. Requires `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_policy(
    State(pg_client): State<PgClient>,
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspacePolicy>,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace policy");

    let workspace = authz.workspace;
    let account_id = authz.account_id;

    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id,
        security: &security,
    };
    let domain::output::ResolvedPolicy {
        policy,
        version,
        created,
    } = policies.create(origin, request.into()).await?;

    let mut conn = pg_client.get_connection().await?;
    let creator = resolve_account_ref(&mut conn, account_id).await?;
    let response =
        WorkspacePolicy::from_model(policy, version, workspace.id, workspace.handle, creator)?;

    // A one-shot body reuses an identical live policy (200) rather than minting a
    // duplicate; everything else creates a new policy (201).
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(response)))
}

fn create_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create policy")
        .description(
            "Creates a structured redaction policy for the workspace. A labels body \
             creates (or reuses) a one-shot policy and returns 200 when an identical \
             one already exists; a template or inline body always creates a new \
             policy and returns 201.",
        )
        .response::<201, Json<WorkspacePolicy>>()
        .response::<200, Json<WorkspacePolicy>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all policies for a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_policies(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ViewPolicies>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspacePoliciesQuery>,
) -> Result<(StatusCode, Json<PoliciesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace policies");

    let workspace = authz.workspace;

    let page = policies
        .list(workspace.id, pagination.into_cursor(), query.kind)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        policy_count = page.items.len(),
        "Workspace policies listed",
    );

    // The list carries only metadata; the definition is loaded only by the
    // single-policy endpoint, so a page stays small.
    let page = PoliciesPage::from_cursor_page(page, |wc| {
        WorkspacePolicySummary::from_model(
            wc.item,
            workspace.id,
            workspace.handle.clone(),
            wc.account.into(),
        )
    });

    Ok((StatusCode::OK, Json(page)))
}

fn list_policies_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List policies")
        .description(
            "Returns the workspace's policies. The optional `kind` query parameter \
             narrows to a single kind (`authored` or `oneshot`).",
        )
        .response::<200, Json<PoliciesPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace policy.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_id = %path_params.policy_id,
    )
)]
async fn read_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ViewPolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace policy");

    let workspace = authz.workspace;

    let (found, version) = policies.find(workspace.id, path_params.policy_id).await?;

    let response = WorkspacePolicy::from_model(
        found.item,
        version,
        workspace.id,
        workspace.handle,
        found.account.into(),
    )?;

    tracing::debug!(target: TRACING_TARGET, "Workspace policy read");
    Ok((StatusCode::OK, Json(response)))
}

fn read_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get policy")
        .description("Returns a single policy.")
        .response::<200, Json<WorkspacePolicy>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace policy.
///
/// All fields are optional; replacing the definition replaces the whole policy
/// body. One-shot policies are immutable, so editing one is rejected. Requires
/// `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_id = %path_params.policy_id,
    )
)]
async fn update_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspacePolicy>,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace policy");

    let workspace = authz.workspace;
    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };

    let (found, version) = policies
        .update(origin, path_params.policy_id, request.into())
        .await?;

    let response = WorkspacePolicy::from_model(
        found.item,
        version,
        workspace.id,
        workspace.handle,
        found.account.into(),
    )?;

    Ok((StatusCode::OK, Json(response)))
}

fn update_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update policy")
        .description("Updates policy fields. Replacing the definition replaces the whole body.")
        .response::<200, Json<WorkspacePolicy>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace policy.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_id = %path_params.policy_id,
    )
)]
async fn delete_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace policy");

    let workspace = authz.workspace;
    let origin = event::EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };

    policies.delete(origin, path_params.policy_id).await?;

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

/// Returns routes for workspace policy management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/policies",
            post_with(create_policy, create_policy_docs)
                .get_with(list_policies, list_policies_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/policies/{policyId}",
            get_with(read_policy, read_policy_docs)
                .patch_with(update_policy, update_policy_docs)
                .delete_with(delete_policy, delete_policy_docs),
        )
        .with_path_items(|item| item.tag("Policies"))
}
