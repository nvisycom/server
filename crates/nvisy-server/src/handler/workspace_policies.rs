//! Workspace policy management handlers.
//!
//! Policies are structured redaction governance documents (the engine's Policy
//! type) consumed by the redaction pipeline. A policy describes what to redact
//! and how — plain config, carrying no sensitive content — so the definition is
//! stored as plaintext JSONB on its version row, scoped to a workspace.
//!
//! These handlers are thin: they authorize, parse the request, delegate the
//! policy rules to [`PolicyService`], and map the result to a response. The
//! domain logic lives in that service.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;

use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspacePolicy, CursorPagination, UpdateWorkspacePolicy, WorkspacePolicyPathParams,
};
use crate::handler::response::{PoliciesPage, WorkspacePolicy, WorkspacePolicySummary};
use crate::handler::utility::resolve_account_ref;
use crate::response::{ErrorResponse, Result};
use crate::service::{EventOrigin, PolicyService, ResolvedPolicy, ServiceState};

/// Tracing target for workspace policy operations.
const TRACING_TARGET: &str = "nvisy_server::handler::policies";

/// Creates a new workspace policy.
///
/// The request body carries a structured policy definition; its name and
/// description drive the stored record unless overridden. A labels body creates
/// (or reuses) a temporary one-shot policy. Requires `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_policy(
    State(pg_client): State<PgClient>,
    State(policies): State<PolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspacePolicy>,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace policy");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let mut conn = pg_client.get_connection().await?;

    let origin = EventOrigin {
        workspace_id: workspace.id,
        account_id,
        security: &security,
    };
    let ResolvedPolicy {
        policy,
        version,
        created,
    } = policies.create(&mut conn, origin, request).await?;

    let creator = resolve_account_ref(&mut conn, account_id).await?;
    let response = WorkspacePolicy::from_model(policy, version, workspace.slug, creator)?;

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
             creates (or reuses) a temporary one-shot policy and returns 200 when an \
             identical one already exists; a template or inline body always creates a \
             new policy and returns 201.",
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
    State(pg_client): State<PgClient>,
    State(policies): State<PolicyService>,
    authz: Authorized<markers::ViewPolicies>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<PoliciesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace policies");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = policies
        .list(&mut conn, workspace.id, pagination.into_cursor())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        policy_count = page.items.len(),
        "Workspace policies listed",
    );

    // The list carries only metadata; the definition is loaded only by the
    // single-policy endpoint, so a page stays small.
    let page = PoliciesPage::from_cursor_page(page, |wc| {
        WorkspacePolicySummary::from_model(wc.item, workspace.slug.clone(), wc.account.into())
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn read_policy(
    State(pg_client): State<PgClient>,
    State(policies): State<PolicyService>,
    authz: Authorized<markers::ViewPolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace policy");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let (found, version) = policies
        .find(&mut conn, workspace.id, &path_params.policy_slug)
        .await?;

    let response =
        WorkspacePolicy::from_model(found.item, version, workspace.slug, found.account.into())?;

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
/// body and, for a one-shot, promotes it. Requires `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn update_policy(
    State(pg_client): State<PgClient>,
    State(policies): State<PolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspacePolicy>,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace policy");

    let workspace = authz.workspace;
    let origin = EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };
    let mut conn = pg_client.get_connection().await?;

    let (found, version) = policies
        .update(&mut conn, origin, &path_params.policy_slug, request)
        .await?;

    let response =
        WorkspacePolicy::from_model(found.item, version, workspace.slug, found.account.into())?;

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
        policy_slug = %path_params.policy_slug,
    )
)]
async fn delete_policy(
    State(pg_client): State<PgClient>,
    State(policies): State<PolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace policy");

    let workspace = authz.workspace;
    let origin = EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };
    let mut conn = pg_client.get_connection().await?;

    policies
        .delete(&mut conn, origin, &path_params.policy_slug)
        .await?;

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

/// Promotes a one-shot policy to an authored one.
///
/// Makes the policy authored and clears its dedup hash, so it appears in the
/// default list and can be attached to a pipeline. A no-op on an already-authored
/// policy. Requires `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn promote_policy(
    State(pg_client): State<PgClient>,
    State(policies): State<PolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Promoting workspace policy");

    let workspace = authz.workspace;
    let origin = EventOrigin {
        workspace_id: workspace.id,
        account_id: authz.account_id,
        security: &security,
    };
    let mut conn = pg_client.get_connection().await?;

    let (found, version) = policies
        .promote(&mut conn, origin, &path_params.policy_slug)
        .await?;

    let response =
        WorkspacePolicy::from_model(found.item, version, workspace.slug, found.account.into())?;

    Ok((StatusCode::OK, Json(response)))
}

fn promote_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Promote policy")
        .description(
            "Promotes a temporary (one-shot) policy to a permanent one, so it \
             appears in the list and can be attached to a pipeline.",
        )
        .response::<200, Json<WorkspacePolicy>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
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
        .api_route(
            "/workspaces/{workspaceSlug}/policies/{policySlug}/promote/",
            post_with(promote_policy, promote_policy_docs),
        )
        .with_path_items(|item| item.tag("Policies"))
}
