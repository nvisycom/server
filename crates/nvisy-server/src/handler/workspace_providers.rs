//! Workspace inference-provider management handlers.
//!
//! A provider is an inference service the platform calls (a language model for
//! chat, a named-entity-recognition model for extraction) — a separate resource
//! from a connection: no transfers, no syncs, no schedule. Its provider config
//! (credentials plus provider-specific settings) is encrypted with
//! workspace-derived keys and never exposed through the API.

use std::time::Duration;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_core::net::EndpointPolicy;
use nvisy_postgres::model::NewWorkspaceProvider;
use nvisy_postgres::query::WorkspaceProviderRepository;
use nvisy_postgres::types::ProviderId;
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, model};
use uuid::Uuid;

use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceProvider, CursorPagination, UpdateWorkspaceProvider,
    WorkspaceProviderPathParams, WorkspaceProvidersQuery,
};
use crate::handler::response::{
    WorkspaceConnectionVerification, WorkspaceProvider, WorkspaceProvidersPage,
};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{
    CryptoService, EventEmitter, EventOrigin, ProviderConfig, ProviderCreated, ProviderDeleted,
    ProviderUpdated, ServiceState, WorkspaceEvent,
};

/// Tracing target for workspace provider operations.
const TRACING_TARGET: &str = "nvisy_server::handler::providers";

/// Upper bound on a provider reachability check, so a hung provider cannot pin
/// the request task indefinitely. A timeout is reported as unreachable.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(30);

/// Creates a new workspace inference provider.
///
/// Returns the provider metadata (without encrypted data). Requires
/// `ManageProviders` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_provider(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(endpoint_policy): State<EndpointPolicy>,
    authz: Authorized<markers::ManageProviders>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceProvider>,
) -> Result<(StatusCode, Json<WorkspaceProvider>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace provider");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Reject a disallowed custom endpoint under the deployment policy before the
    // config is ever stored (SSRF / cleartext-credential guard).
    request.config.validate_endpoints(endpoint_policy).await?;

    // The provider and its model type are derived from the typed config so they
    // can never disagree with it; the full config is encrypted at rest.
    let provider = request.config.provider_id().to_owned();
    let provider_type = request.config.provider_type();
    let encrypted_data = crypto.encrypt_json(workspace.id, &request.config)?;

    let new_provider = NewWorkspaceProvider {
        workspace_id: workspace.id,
        account_id,
        display_name: request.display_name,
        provider,
        provider_type,
        encrypted_data,
        is_active: request.is_active,
        metadata: None,
    };

    // Insert the provider and the outbox event atomically, so the event is never
    // recorded for a create that rolled back, nor lost after a committed one.
    let created = conn
        .transaction(async |conn| {
            let created = conn.create_workspace_provider(new_provider).await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id,
                    security: &security,
                },
                WorkspaceEvent::ProviderCreated(ProviderCreated {
                    provider_id: created.id,
                    provider_name: created.display_name.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(created)
        })
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        provider_id = %ProviderId::from_uuid(created.id),
        provider = %created.provider,
        "WorkspaceProvider created",
    );

    let creator = resolve_account_ref(&mut conn, account_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceProvider::from_model(
            created,
            workspace.slug,
            creator,
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
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all inference providers for a workspace.
///
/// Returns provider metadata (without encrypted data). Requires `ViewProviders`
/// permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_providers(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewProviders>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceProvidersQuery>,
) -> Result<(StatusCode, Json<WorkspaceProvidersPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace providers");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_workspace_providers(workspace.id, pagination.into_cursor(), &query.provider)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        provider_count = page.items.len(),
        "Workspace providers listed",
    );

    Ok((
        StatusCode::OK,
        Json(WorkspaceProvidersPage::from_cursor_page(page, |wp| {
            WorkspaceProvider::from_model(wp.item, workspace.slug.clone(), wp.account.into())
        })),
    ))
}

fn list_providers_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List providers")
        .description(
            "Returns all configured inference providers for the workspace. Only metadata is \
             returned; encrypted credentials are never exposed.",
        )
        .response::<200, Json<WorkspaceProvidersPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace provider.
///
/// Returns provider metadata (without encrypted data). Requires `ViewProviders`
/// permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        provider_id = %path_params.provider_id,
    )
)]
async fn read_provider(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
) -> Result<(StatusCode, Json<WorkspaceProvider>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace provider");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let found = find_provider(&mut conn, workspace.id, path_params.provider_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace provider read");

    Ok((
        StatusCode::OK,
        Json(WorkspaceProvider::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
        )),
    ))
}

fn read_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get provider")
        .description("Returns provider metadata without encrypted credentials.")
        .response::<200, Json<WorkspaceProvider>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace provider.
///
/// Updates provider configuration. Requires `ManageProviders` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        provider_id = %path_params.provider_id,
    )
)]
async fn update_provider(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(endpoint_policy): State<EndpointPolicy>,
    authz: Authorized<markers::ManageProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspaceProvider>,
) -> Result<(StatusCode, Json<WorkspaceProvider>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace provider");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Reject a disallowed custom endpoint on the replacement config before store.
    if let Some(config) = &request.config {
        config.validate_endpoints(endpoint_policy).await?;
    }

    let existing = find_provider(&mut conn, workspace.id, path_params.provider_id)
        .await?
        .item;

    let provider_id = existing.id;
    // The effective post-update name: the new one if the request set it, else the
    // existing name.
    let provider_name = request
        .display_name
        .clone()
        .unwrap_or_else(|| existing.display_name.clone());
    let crypto = crypto.clone();
    conn.transaction(async move |conn| {
        // Re-encrypt the replacement config. A provider's concrete provider is
        // fixed at creation, so a config replacement must keep the same provider —
        // changing it would desync the provider/provider_type columns. Reject a
        // differing provider rather than silently migrate; since the provider is
        // preserved, so is its model type, so provider_type needs no update.
        let (provider, encrypted_data) = match request.config {
            Some(config) => {
                let stored: ProviderConfig =
                    crypto.decrypt_json(workspace.id, &existing.encrypted_data)?;
                if config.provider_id() != stored.provider_id() {
                    return Err(ErrorKind::BadRequest.with_message(
                        "A provider's provider cannot be changed; delete and recreate instead",
                    ));
                }
                (
                    Some(config.provider_id().to_owned()),
                    Some(crypto.encrypt_json(workspace.id, &config)?),
                )
            }
            None => (None, None),
        };

        let update_data = model::UpdateWorkspaceProvider {
            display_name: request.display_name,
            provider,
            is_active: request.is_active,
            encrypted_data,
            ..Default::default()
        };
        conn.update_workspace_provider(provider_id, update_data)
            .await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            WorkspaceEvent::ProviderUpdated(ProviderUpdated {
                provider_id,
                provider_name,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    let found = find_provider(&mut conn, workspace.id, path_params.provider_id).await?;

    tracing::info!(target: TRACING_TARGET, "WorkspaceProvider updated");

    Ok((
        StatusCode::OK,
        Json(WorkspaceProvider::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
        )),
    ))
}

fn update_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update provider")
        .description("Updates provider name or encrypted data.")
        .response::<200, Json<WorkspaceProvider>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace provider.
///
/// Soft-deletes the provider. Requires `ManageProviders` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        provider_id = %path_params.provider_id,
    )
)]
async fn delete_provider(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ManageProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace provider");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let existing = find_provider(&mut conn, workspace.id, path_params.provider_id)
        .await?
        .item;

    conn.transaction(async |conn| {
        conn.delete_workspace_provider(existing.id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            WorkspaceEvent::ProviderDeleted(ProviderDeleted {
                provider_id: existing.id,
                provider_name: existing.display_name.clone(),
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "WorkspaceProvider deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete provider")
        .description("Soft-deletes the provider from the workspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Verifies that a provider is reachable with its stored credentials.
///
/// Decrypts the stored provider config and attempts a lightweight credential
/// check against the provider. Returns `200` with a [`ConnectionVerification`]
/// describing the outcome: a provider that is reachable but rejects the
/// credentials reports `reachable: false` with the reason, rather than an HTTP
/// error. Requires `ViewProviders` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        provider_id = %path_params.provider_id,
    )
)]
async fn verify_provider(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    authz: Authorized<markers::ViewProviders>,
    Path(path_params): Path<WorkspaceProviderPathParams>,
) -> Result<(StatusCode, Json<WorkspaceConnectionVerification>)> {
    tracing::debug!(target: TRACING_TARGET, "Verifying workspace provider");

    let workspace = authz.workspace;

    // Do the DB work up front, then release the connection before the provider I/O
    // below, which reaches an external service with no total timeout.
    let provider = {
        let mut conn = pg_client.get_connection().await?;
        find_provider(&mut conn, workspace.id, path_params.provider_id)
            .await?
            .item
    };

    let config: ProviderConfig = crypto.decrypt_json(workspace.id, &provider.encrypted_data)?;

    let verification = match tokio::time::timeout(VERIFY_TIMEOUT, config.validate()).await {
        Ok(Ok(())) => {
            tracing::info!(target: TRACING_TARGET, "WorkspaceProvider verified");
            WorkspaceConnectionVerification::reachable()
        }
        Ok(Err(err)) => {
            // Log the full error, but return only a safe reason so provider
            // endpoints/keys are not echoed to the client.
            tracing::warn!(target: TRACING_TARGET, error = %err, "WorkspaceProvider verification failed");
            WorkspaceConnectionVerification::unreachable(
                "credentials rejected or provider unreachable",
            )
        }
        Err(_elapsed) => {
            tracing::warn!(target: TRACING_TARGET, "WorkspaceProvider verification timed out");
            WorkspaceConnectionVerification::unreachable("provider did not respond in time")
        }
    };

    Ok((StatusCode::OK, Json(verification)))
}

fn verify_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Verify provider")
        .description("Checks whether the provider is reachable with its stored credentials.")
        .response::<200, Json<WorkspaceConnectionVerification>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a provider within a workspace by id, with its creator, or returns a
/// NotFound error.
async fn find_provider(
    conn: &mut PgConn,
    workspace_id: Uuid,
    provider_id: ProviderId,
) -> Result<nvisy_postgres::types::WithAccountRef<nvisy_postgres::model::WorkspaceProvider>> {
    conn.find_provider_in_workspace_with_creator(workspace_id, provider_id.as_uuid())
        .await?
        .ok_or_else(|| Error::not_found("provider"))
}

/// Returns routes for workspace provider management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/providers/",
            post_with(create_provider, create_provider_docs)
                .get_with(list_providers, list_providers_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/providers/{providerId}/",
            get_with(read_provider, read_provider_docs)
                .patch_with(update_provider, update_provider_docs)
                .delete_with(delete_provider, delete_provider_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/providers/{providerId}/verify/",
            post_with(verify_provider, verify_provider_docs),
        )
        .with_path_items(|item| item.tag("Providers"))
}
