//! Workspace inference-provider management handlers.
//!
//! A provider is an inference service the platform calls (a language model for
//! chat, a named-entity-recognition model for extraction) — a separate resource
//! from a connection: no transfers, no syncs, no schedule. Its provider config
//! (credentials plus provider-specific settings) is encrypted with
//! workspace-derived keys and never exposed through the API. The CRUD orchestration
//! lives in [`WorkspaceProviderService`]; the handler resolves the service, maps
//! the result to a response, and owns the reachability check.

use std::time::Duration;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceProvider, CursorPagination, UpdateWorkspaceProvider,
    WorkspaceProviderPathParams, WorkspaceProvidersQuery,
};
use crate::handler::response::{
    WorkspaceConnectionVerification, WorkspaceProvider, WorkspaceProvidersPage, WorkspaceRef,
};
use crate::response::Result;
use crate::service::{CryptoService, ProviderConfig, ServiceState};

/// Tracing target for workspace provider operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_providers";

/// Upper bound on a provider reachability check, so a hung provider cannot pin
/// the request task indefinitely. A timeout is reported as unreachable.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(30);

/// Creates a new workspace inference provider.
///
/// Returns the provider metadata (without encrypted data). Requires
/// `ManageProviders` permission.
#[tracing::instrument(skip_all)]
async fn create_provider(
    State(providers): State<domain::WorkspaceProviderService>,
    authz: Authorized<markers::ManageProviders>,
    ValidateJson(request): ValidateJson<CreateWorkspaceProvider>,
) -> Result<(StatusCode, Json<WorkspaceProvider>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace provider");

    let workspace = &authz.workspace;
    let created = providers.create(authz.origin(), request.into()).await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceProvider::from_model(
            created.item,
            WorkspaceRef::from(workspace),
            created.account.into(),
        )),
    ))
}

fn create_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create provider")
        .description(
            "Creates a new inference provider for the workspace. WorkspaceProvider data is encrypted and \
             stored securely. The response includes provider metadata but never exposes the \
             encrypted credentials.",
        )
        .response::<201, Json<WorkspaceProvider>>()
}

/// Lists all inference providers for a workspace.
///
/// Returns provider metadata (without encrypted data). Requires `ViewProviders`
/// permission.
#[tracing::instrument(skip_all)]
async fn list_providers(
    State(providers): State<domain::WorkspaceProviderService>,
    authz: Authorized<markers::ViewProviders>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceProvidersQuery>,
) -> Result<Json<WorkspaceProvidersPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace providers");

    let workspace = &authz.workspace;
    let page = providers
        .list(workspace.id, pagination.into_cursor(), &query.provider)
        .await?;

    Ok(Json(WorkspaceProvidersPage::from_cursor_page(page, |wp| {
        WorkspaceProvider::from_model(wp.item, WorkspaceRef::from(workspace), wp.account.into())
    })))
}

fn list_providers_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List providers")
        .description(
            "Returns all configured inference providers for the workspace. Only metadata is \
             returned; encrypted credentials are never exposed.",
        )
        .response::<200, Json<WorkspaceProvidersPage>>()
}

/// Retrieves a specific workspace provider.
///
/// Returns provider metadata (without encrypted data). Requires `ViewProviders`
/// permission.
#[tracing::instrument(
    skip_all,
    fields(
        provider_id = %path_params.provider_id,
    )
)]
async fn read_provider(
    State(providers): State<domain::WorkspaceProviderService>,
    authz: Authorized<markers::ViewProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
) -> Result<Json<WorkspaceProvider>> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace provider");

    let workspace = &authz.workspace;
    let found = providers
        .find(workspace.id, path_params.provider_id)
        .await?;

    Ok(Json(WorkspaceProvider::from_model(
        found.item,
        WorkspaceRef::from(workspace),
        found.account.into(),
    )))
}

fn read_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get provider")
        .description("Returns provider metadata without encrypted credentials.")
        .response::<200, Json<WorkspaceProvider>>()
}

/// Updates a workspace provider.
///
/// Updates provider configuration. Requires `ManageProviders` permission.
#[tracing::instrument(
    skip_all,
    fields(
        provider_id = %path_params.provider_id,
    )
)]
async fn update_provider(
    State(providers): State<domain::WorkspaceProviderService>,
    authz: Authorized<markers::ManageProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspaceProvider>,
) -> Result<Json<WorkspaceProvider>> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace provider");

    let workspace = &authz.workspace;
    let found = providers
        .update(authz.origin(), path_params.provider_id, request.into())
        .await?;

    Ok(Json(WorkspaceProvider::from_model(
        found.item,
        WorkspaceRef::from(workspace),
        found.account.into(),
    )))
}

fn update_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update provider")
        .description("Updates provider name or encrypted data.")
        .response::<200, Json<WorkspaceProvider>>()
}

/// Deletes a workspace provider.
///
/// Soft-deletes the provider. Requires `ManageProviders` permission.
#[tracing::instrument(
    skip_all,
    fields(
        provider_id = %path_params.provider_id,
    )
)]
async fn delete_provider(
    State(providers): State<domain::WorkspaceProviderService>,
    authz: Authorized<markers::ManageProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace provider");

    providers
        .delete(authz.origin(), path_params.provider_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn delete_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete provider")
        .description("Soft-deletes the provider from the workspace.")
        .response::<204, ()>()
}

/// Verifies that a provider is reachable with its stored credentials.
///
/// Decrypts the stored provider config and attempts a lightweight credential
/// check against the provider. Returns `200` with a [`WorkspaceConnectionVerification`]
/// describing the outcome: a provider that is reachable but rejects the
/// credentials reports `reachable: false` with the reason, rather than an HTTP
/// error. Requires `ViewProviders` permission.
#[tracing::instrument(
    skip_all,
    fields(
        provider_id = %path_params.provider_id,
    )
)]
async fn verify_provider(
    State(providers): State<domain::WorkspaceProviderService>,
    State(crypto): State<CryptoService>,
    authz: Authorized<markers::ViewProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
) -> Result<Json<WorkspaceConnectionVerification>> {
    tracing::debug!(target: TRACING_TARGET, "Verifying workspace provider");

    let workspace = &authz.workspace;

    // Load the provider through the service, then run the external I/O below with
    // no pooled connection held (the check reaches an external service with no
    // total timeout).
    let provider = providers
        .find(workspace.id, path_params.provider_id)
        .await?
        .item;

    let config: ProviderConfig = crypto.decrypt_json(workspace.id, &provider.encrypted_data)?;

    let verification = match tokio::time::timeout(VERIFY_TIMEOUT, config.validate()).await {
        Ok(Ok(())) => {
            tracing::info!(target: TRACING_TARGET, "Provider verified");
            WorkspaceConnectionVerification::reachable()
        }
        Ok(Err(err)) => {
            // Log the full error, but return only a safe reason so provider
            // endpoints/keys are not echoed to the client.
            tracing::warn!(target: TRACING_TARGET, error = %err, "Provider verification failed");
            WorkspaceConnectionVerification::unreachable(
                "credentials rejected or provider unreachable",
            )
        }
        Err(_elapsed) => {
            tracing::warn!(target: TRACING_TARGET, "Provider verification timed out");
            WorkspaceConnectionVerification::unreachable("provider did not respond in time")
        }
    };

    Ok(Json(verification))
}

fn verify_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Verify provider")
        .description("Checks whether the provider is reachable with its stored credentials.")
        .response::<200, Json<WorkspaceConnectionVerification>>()
}

/// Returns routes for workspace provider management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/providers",
            post_with(create_provider, create_provider_docs)
                .get_with(list_providers, list_providers_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/providers/{providerId}",
            get_with(read_provider, read_provider_docs)
                .patch_with(update_provider, update_provider_docs)
                .delete_with(delete_provider, delete_provider_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/providers/{providerId}/verify",
            post_with(verify_provider, verify_provider_docs),
        )
        .with_path_items(|item| item.tag("Providers"))
}
