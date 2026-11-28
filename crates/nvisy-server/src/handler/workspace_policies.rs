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
use nvisy_postgres::query::PolicyWithVersion;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspacePolicy, CursorPagination, PromoteWorkspacePolicy, UpdateWorkspacePolicy,
    WorkspacePoliciesQuery, WorkspacePolicyPathParams,
};
use crate::handler::response::{
    PoliciesPage, WorkspacePolicy, WorkspacePolicySummary, WorkspaceRef,
};
use crate::response::Result;
use crate::service::ServiceState;

/// Tracing target for workspace policy operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_policies";

/// Creates a new authored workspace policy.
///
/// The request body carries a template or an inline definition; its name and
/// description drive the stored record unless overridden. Requires
/// `ManagePolicies` permission.
#[tracing::instrument(skip_all)]
async fn create_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    ValidateJson(request): ValidateJson<CreateWorkspacePolicy>,
) -> Result<(StatusCode, Json<WorkspacePolicy>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace policy");

    let workspace = &authz.workspace;

    let origin = authz.origin();
    let PolicyWithVersion { policy, version } = policies.create(origin, request.into()).await?;

    let creator = authz.account.clone().into();
    let response =
        WorkspacePolicy::from_model(policy, version, WorkspaceRef::from(workspace), creator)?;

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create policy")
        .description(
            "Creates an authored redaction policy for the workspace from a template \
             or an inline definition. To detect by a list of labels without \
             authoring a policy, start a detection with a `labels` source instead.",
        )
        .response::<201, Json<WorkspacePolicy>>()
}

/// Lists all policies for a workspace.
#[tracing::instrument(skip_all)]
async fn list_policies(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ViewPolicies>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspacePoliciesQuery>,
) -> Result<Json<PoliciesPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace policies");

    let workspace = &authz.workspace;

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
            WorkspaceRef::from(workspace),
            wc.account.into(),
        )
    });

    Ok(Json(page))
}

fn list_policies_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List policies")
        .description(
            "Returns the workspace's policies. The optional `kind` query parameter \
             narrows to a single kind (`authored` or `derived`).",
        )
        .response::<200, Json<PoliciesPage>>()
}

/// Retrieves a specific workspace policy.
#[tracing::instrument(
    skip_all,
    fields(
        policy_id = %path_params.policy_id,
    )
)]
async fn read_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ViewPolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
) -> Result<Json<WorkspacePolicy>> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace policy");

    let workspace = &authz.workspace;

    let (found, version) = policies.find(workspace.id, path_params.policy_id).await?;

    let response = WorkspacePolicy::from_model(
        found.item,
        version,
        WorkspaceRef::from(workspace),
        found.account.into(),
    )?;

    tracing::debug!(target: TRACING_TARGET, "Workspace policy read");
    Ok(Json(response))
}

fn read_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get policy")
        .description("Returns a single policy.")
        .response::<200, Json<WorkspacePolicy>>()
}

/// Updates a workspace policy.
///
/// All fields are optional; replacing the definition replaces the whole policy
/// body. Derived policies are immutable, so editing one is rejected. Requires
/// `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        policy_id = %path_params.policy_id,
    )
)]
async fn update_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspacePolicy>,
) -> Result<Json<WorkspacePolicy>> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace policy");

    let workspace = &authz.workspace;
    let origin = authz.origin();

    let (found, version) = policies
        .update(origin, path_params.policy_id, request.into())
        .await?;

    let response = WorkspacePolicy::from_model(
        found.item,
        version,
        WorkspaceRef::from(workspace),
        found.account.into(),
    )?;

    Ok(Json(response))
}

fn update_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update policy")
        .description("Updates policy fields. Replacing the definition replaces the whole body.")
        .response::<200, Json<WorkspacePolicy>>()
}

/// Promotes a derived policy to an authored one.
///
/// A derived (ad-hoc) policy is immutable and content-addressed; promoting it
/// clears its content hash and makes it an editable, reusable authored policy
/// selectable for detections, keeping its id and current definition. Rejected for
/// an already-authored policy. Requires `ManagePolicies` permission.
#[tracing::instrument(
    skip_all,
    fields(
        policy_id = %path_params.policy_id,
    )
)]
async fn promote_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
    ValidateJson(request): ValidateJson<PromoteWorkspacePolicy>,
) -> Result<Json<WorkspacePolicy>> {
    tracing::debug!(target: TRACING_TARGET, "Promoting workspace policy");

    let workspace = &authz.workspace;
    let origin = authz.origin();

    let (found, version) = policies
        .promote(origin, path_params.policy_id, request.display_name)
        .await?;

    let response = WorkspacePolicy::from_model(
        found.item,
        version,
        WorkspaceRef::from(workspace),
        found.account.into(),
    )?;

    Ok(Json(response))
}

fn promote_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Promote policy")
        .description(
            "Promotes a derived (ad-hoc) policy to an authored one: clears its \
             content hash and makes it an editable, reusable policy selectable for \
             detections, keeping its id and current definition. A `displayName` is required and \
             replaces the policy's generated hash name. Returns 400 if the policy is \
             already authored.",
        )
        .response::<200, Json<WorkspacePolicy>>()
}

/// Deletes a workspace policy.
#[tracing::instrument(
    skip_all,
    fields(
        policy_id = %path_params.policy_id,
    )
)]
async fn delete_policy(
    State(policies): State<domain::WorkspacePolicyService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<WorkspacePolicyPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace policy");

    let origin = authz.origin();

    policies.delete(origin, path_params.policy_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn delete_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete policy")
        .description("Soft-deletes the policy from the workspace.")
        .response::<204, ()>()
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
        .api_route(
            "/workspaces/{workspaceId}/policies/{policyId}/promote",
            post_with(promote_policy, promote_policy_docs),
        )
        .with_path_items(|item| item.tag("Policies"))
}
