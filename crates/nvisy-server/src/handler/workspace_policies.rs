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
use nvisy_postgres::model::{
    NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy, WorkspacePolicyVersion,
};
use nvisy_postgres::query::{WorkspacePolicyRepository, WorkspacePolicyVersionRepository};
use nvisy_postgres::types::{Handle, PolicyKind, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreatePolicy, CursorPagination, PolicyBody, PolicyPathParams, UpdatePolicy,
};
use crate::handler::response::{PoliciesPage, Policy, PolicySummary};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{
    CryptoService, EventEmitter, EventOrigin, PolicyCreated, PolicyDeleted, PolicyPromoted,
    PolicyUpdated, ServiceState, WorkspaceEvent,
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_policy(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    authz: Authorized<markers::ManagePolicies>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreatePolicy>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace policy");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let mut conn = pg_client.get_connection().await?;

    // A one-shot (labels) body is content-addressed: it mints (or reuses) a
    // temporary policy with a server-generated, hash-derived slug and name. A
    // template or inline body is a normal, permanent policy the caller names.
    let (policy, version, status) = match request.body.oneshot_content_hash() {
        Some(content_hash) => {
            create_oneshot(
                &mut conn,
                &crypto,
                &security,
                workspace.id,
                account_id,
                request.body,
                content_hash,
            )
            .await?
        }
        None => {
            create_authored(
                &mut conn,
                &crypto,
                &security,
                workspace.id,
                account_id,
                request,
            )
            .await?
        }
    };

    // The creator is the authenticated caller; resolve their handle directly.
    let creator = resolve_account_ref(&mut conn, account_id).await?;

    let response = Policy::from_model(policy, version, workspace.slug, creator, &crypto)?;

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
        .response::<201, Json<Policy>>()
        .response::<200, Json<Policy>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Creates an authored (template or inline) policy: a new permanent row with a
/// caller-supplied slug, its first version, and a creation event, in one
/// transaction. Returns `201`.
async fn create_authored(
    conn: &mut PgConn,
    crypto: &CryptoService,
    security: &SecurityContext,
    workspace_id: Uuid,
    account_id: Uuid,
    request: CreatePolicy,
) -> Result<(WorkspacePolicy, WorkspacePolicyVersion, StatusCode)> {
    let slug = request.slug.ok_or_else(|| {
        ErrorKind::BadRequest.with_message("A slug is required for this policy body")
    })?;

    let definition = request.body.into_definition("");
    let display_name = request
        .display_name
        .unwrap_or_else(|| definition.name.to_string());
    let description = request
        .description
        .or_else(|| definition.description.clone().map(Into::into));
    let encrypted = crypto.encrypt_json(workspace_id, &definition)?;

    let new_policy = NewWorkspacePolicy {
        workspace_id,
        account_id,
        slug,
        display_name,
        description,
        kind: PolicyKind::Authored,
        content_hash: None,
        metadata: None,
    };

    let created = conn
        .transaction(async |conn| {
            let created = conn
                .create_workspace_policy(new_policy, encrypted, None)
                .await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id,
                    account_id,
                    security,
                },
                WorkspaceEvent::PolicyCreated(PolicyCreated {
                    policy_id: created.policy.id,
                    policy_slug: created.policy.slug.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(created)
        })
        .await?;

    tracing::info!(target: TRACING_TARGET, policy_slug = %created.policy.slug, "Policy created");
    Ok((created.policy, created.version, StatusCode::CREATED))
}

/// Creates or reuses a one-shot policy from a labels body. The policy is
/// content-addressed by `content_hash`, so an identical live one-shot is reused
/// (returned `200`) rather than duplicated; a fresh one is created (`201`) with a
/// hash-derived slug and name and a creation event.
async fn create_oneshot(
    conn: &mut PgConn,
    crypto: &CryptoService,
    security: &SecurityContext,
    workspace_id: Uuid,
    account_id: Uuid,
    body: PolicyBody,
    content_hash: Vec<u8>,
) -> Result<(WorkspacePolicy, WorkspacePolicyVersion, StatusCode)> {
    let slug = Handle::parse(oneshot_slug(&content_hash)).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to generate a one-shot policy slug")
            .with_context(err.to_string())
    })?;
    let display_name = oneshot_display_name(&content_hash);

    let definition = body.into_definition(&display_name);
    let encrypted = crypto.encrypt_json(workspace_id, &definition)?;

    let new_policy = NewWorkspacePolicy {
        workspace_id,
        account_id,
        slug,
        display_name,
        description: None,
        kind: PolicyKind::Oneshot,
        content_hash: Some(content_hash.clone()),
        metadata: None,
    };

    let resolved = conn
        .find_or_create_oneshot_policy(new_policy, content_hash, encrypted, None)
        .await?;

    // Only a fresh row is a creation: reusing an existing one-shot records no event.
    if resolved.created {
        conn.emit_event(
            EventOrigin {
                workspace_id,
                account_id,
                security,
            },
            WorkspaceEvent::PolicyCreated(PolicyCreated {
                policy_id: resolved.policy.policy.id,
                policy_slug: resolved.policy.policy.slug.clone(),
            }),
        )
        .await?;
        tracing::info!(target: TRACING_TARGET, policy_slug = %resolved.policy.policy.slug, "One-shot policy created");
    }

    let status = if resolved.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((resolved.policy.policy, resolved.policy.version, status))
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
    authz: Authorized<markers::ViewPolicies>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<PoliciesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace policies");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_workspace_policies(workspace.id, pagination.into_cursor())
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn read_policy(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    authz: Authorized<markers::ViewPolicies>,
    Path(path_params): Path<PolicyPathParams>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace policy");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let found = find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;
    let version = current_version(&mut conn, workspace.id, &found.item).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace policy read");

    Ok((
        StatusCode::OK,
        Json(Policy::from_model(
            found.item,
            version,
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn update_policy(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<PolicyPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdatePolicy>,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace policy");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let mut conn = pg_client.get_connection().await?;

    // Confirm the policy exists in this workspace, and load its current version
    // (its definition carries the server-owned template origin).
    let existing = find_policy(&mut conn, workspace.id, &path_params.policy_slug)
        .await?
        .item;
    let current = current_version(&mut conn, workspace.id, &existing).await?;

    // A replaced body keeps the policy's server-owned template origin: the caller
    // authored new rules, but where the policy came from is provenance the client
    // cannot set or clear. Carry the current version's origin forward.
    let encrypted_definition = match request.definition {
        Some(draft) => {
            let template = crypto
                .decrypt_json::<PolicyDefinition>(workspace.id, &current.definition)?
                .template;
            let definition = draft.into_definition(template);
            Some(crypto.encrypt_json(workspace.id, &definition)?)
        }
        None => None,
    };

    let policy_id = existing.id;
    let policy_slug = existing.slug.clone();

    // A definition change mints a new version; a label-only change mutates the
    // logical row in place. Either way, record the outbox event atomically with
    // the write.
    conn.transaction(async |conn| {
        if let Some(encrypted) = encrypted_definition {
            conn.create_policy_version(workspace.id, policy_id, account_id, encrypted, None)
                .await?;
        }
        conn.update_workspace_policy(
            policy_id,
            UpdateWorkspacePolicy {
                display_name: request.display_name,
                description: request.description,
                ..Default::default()
            },
        )
        .await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            WorkspaceEvent::PolicyUpdated(PolicyUpdated {
                policy_id,
                policy_slug,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    let found = find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;
    let version = current_version(&mut conn, workspace.id, &found.item).await?;

    let response = Policy::from_model(
        found.item,
        version,
        workspace.slug,
        found.account.into(),
        &crypto,
    )?;

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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        policy_slug = %path_params.policy_slug,
    )
)]
async fn delete_policy(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<PolicyPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace policy");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let mut conn = pg_client.get_connection().await?;

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
                account_id,
                security: &security,
            },
            WorkspaceEvent::PolicyDeleted(PolicyDeleted {
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
    State(crypto): State<CryptoService>,
    authz: Authorized<markers::ManagePolicies>,
    Path(path_params): Path<PolicyPathParams>,
    security: SecurityContext,
) -> Result<(StatusCode, Json<Policy>)> {
    tracing::debug!(target: TRACING_TARGET, "Promoting workspace policy");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let mut conn = pg_client.get_connection().await?;

    let existing = find_policy(&mut conn, workspace.id, &path_params.policy_slug)
        .await?
        .item;
    let policy_id = existing.id;
    let policy_slug = existing.slug.clone();

    // Make it authored (clearing the dedup hash) and record the event atomically.
    conn.transaction(async |conn| {
        conn.update_workspace_policy(
            policy_id,
            UpdateWorkspacePolicy {
                kind: Some(PolicyKind::Authored),
                content_hash: Some(None),
                ..Default::default()
            },
        )
        .await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            WorkspaceEvent::PolicyPromoted(PolicyPromoted {
                policy_id,
                policy_slug,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    let found = find_policy(&mut conn, workspace.id, &path_params.policy_slug).await?;
    let version = current_version(&mut conn, workspace.id, &found.item).await?;

    let response = Policy::from_model(
        found.item,
        version,
        workspace.slug,
        found.account.into(),
        &crypto,
    )?;

    tracing::info!(target: TRACING_TARGET, "Policy promoted");

    Ok((StatusCode::OK, Json(response)))
}

fn promote_policy_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Promote policy")
        .description(
            "Promotes a temporary (one-shot) policy to a permanent one, so it \
             appears in the list and can be attached to a pipeline.",
        )
        .response::<200, Json<Policy>>()
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

/// Loads a policy's current version (the one whose definition the engine
/// consumes). A live policy always has a current version.
async fn current_version(
    conn: &mut PgConn,
    workspace_id: Uuid,
    policy: &WorkspacePolicy,
) -> Result<WorkspacePolicyVersion> {
    let version_id = policy
        .current_version_id
        .ok_or_else(|| Error::not_found("policy_version"))?;
    conn.find_policy_version(workspace_id, version_id)
        .await?
        .ok_or_else(|| Error::not_found("policy_version"))
}

/// The slug for a one-shot policy, e.g. `oneshot-1f0a3c8b9d2e`.
///
/// Derived from the content hash, so the same one-shot always maps to the same
/// slug and dedup reuses its row rather than colliding. The 12-hex prefix of the
/// hash satisfies the slug format (lowercase alphanumeric with single internal
/// dashes) and length (3-32).
fn oneshot_slug(content_hash: &[u8]) -> String {
    format!("oneshot-{}", hex_prefix(content_hash, 6))
}

/// The display name for a one-shot policy, e.g. `Quick redaction 1f0a3c8b9d2e`.
///
/// Derived from the content hash (distinct hashes give distinct names), so it
/// never violates the per-workspace display-name uniqueness invariant while dedup
/// keeps one row per distinct content.
fn oneshot_display_name(content_hash: &[u8]) -> String {
    format!("Quick redaction {}", hex_prefix(content_hash, 6))
}

/// Lowercase hex of the first `bytes` bytes of `content_hash`.
fn hex_prefix(content_hash: &[u8], bytes: usize) -> String {
    content_hash
        .iter()
        .take(bytes)
        .map(|b| format!("{b:02x}"))
        .collect()
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
