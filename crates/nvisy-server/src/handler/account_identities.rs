//! Account identity (credential) management: the authenticated account's
//! sign-in methods.
//!
//! An account authenticates through one or more identities — a local password
//! and/or linked OIDC providers — and this module manages them uniformly under
//! the singular `/account/` self-resource:
//!
//! - `GET    /account/identities` — list the account's sign-in methods.
//! - `PUT    /account/identities/password` — set or change the password.
//! - `DELETE /account/identities/password` — remove the password.
//! - `POST   /account/identities/{provider}` — link a provider.
//! - `DELETE /account/identities/{provider}` — unlink a provider.
//!
//! Linking a provider is the OIDC redirect flow in [`auth_oidc`](super::auth_oidc)
//! (it needs a browser round-trip); everything else is a plain authenticated
//! request here. Two invariants hold across the deletes: an account may never
//! lose its **last** identity (it must keep a way to sign in), and adding a
//! credential from a merely-live session is refused (a stolen session must not
//! plant a durable credential — see the step-up proof).

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with, put_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::types::IdentityProvider;

use crate::domain::{AccountIdentityService, ReauthVerified};
use crate::extract::{AuthState, Json, Path, ValidateJson};
use crate::handler::request::{IdentityPathParams, SetPassword};
use crate::handler::response::AccountIdentities;
use crate::response::{ErrorKind, ErrorResponse, Result};
use crate::service::{OidcService, ServiceState};

/// Tracing target for identity operations.
const TRACING_TARGET: &str = "nvisy_server::handler::identities";

/// Lists the authenticated account's sign-in methods.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn list_identities(
    State(identities): State<AccountIdentityService>,
    auth_state: AuthState,
) -> Result<Json<AccountIdentities>> {
    tracing::debug!(target: TRACING_TARGET, "Listing account identities");

    let identities = identities
        .list(auth_state.account_id)
        .await?
        .into_iter()
        .collect();
    Ok(Json(identities))
}

fn list_identities_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List sign-in methods")
        .description("Returns the authenticated account's identities: its password and any linked providers.")
        .response::<200, Json<AccountIdentities>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Sets or changes the authenticated account's password.
///
/// The authorization differs by case and stays here because it is transport: a
/// change verifies the current password (owned by the service); a *first*-set
/// requires a step-up re-authentication proof — consumed single-use against
/// NATS-KV here — because a merely-live session must not mint a durable new
/// credential. The service owns strength-checking, hashing, and the write.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn set_password(
    State(identities): State<AccountIdentityService>,
    State(oidc): State<OidcService>,
    auth_state: AuthState,
    ValidateJson(request): ValidateJson<SetPassword>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Setting account password");

    let account_id = auth_state.account_id;

    if identities.has_password(account_id).await? {
        identities
            .change_password(
                account_id,
                request.current_password.as_deref(),
                &request.new_password,
            )
            .await?;
    } else {
        // Setting a first password creates a new, durable credential, so a live
        // session is not enough. Require a fresh step-up re-authentication proof
        // (from the OIDC reauth endpoint), consumed single-use here before the
        // service writes the credential.
        let proof = request.reauth_proof.as_deref().ok_or_else(|| {
            ErrorKind::Unauthorized
                .with_message("Re-authentication required to set a password")
                .with_resource("account")
        })?;
        oidc.consume_reauth_proof(account_id, proof).await?;
        identities
            .set_first_password(account_id, &request.new_password, ReauthVerified)
            .await?;
    }

    tracing::info!(target: TRACING_TARGET, "Account password set");
    Ok(StatusCode::NO_CONTENT)
}

fn set_password_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Set or change password")
        .description(
            "Sets or changes the account's password. Changing an existing password requires the \
             current password; setting a first password on an account that has none requires a \
             step-up re-authentication proof.",
        )
        .response_with::<204, (), _>(|res| res.description("Password set."))
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Removes the authenticated account's password identity.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn delete_password(
    State(identities): State<AccountIdentityService>,
    auth_state: AuthState,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Removing account password");
    identities
        .remove_identity(auth_state.account_id, IdentityProvider::Password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn delete_password_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Remove password")
        .description(
            "Removes the account's password, leaving it able to sign in only through its linked \
             providers. Refused if the password is the account's only sign-in method.",
        )
        .response_with::<204, (), _>(|res| res.description("Password removed."))
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Unlinks a provider from the authenticated account.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, provider = ?path_params.provider))]
async fn unlink_provider(
    State(identities): State<AccountIdentityService>,
    auth_state: AuthState,
    Path(path_params): Path<IdentityPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Unlinking provider");

    // The password is managed through its own endpoint; this route is for OIDC
    // providers only.
    if !path_params.provider.is_oidc() {
        return Err(ErrorKind::BadRequest
            .with_message("Use the password endpoint to remove a password")
            .with_resource("account_identity"));
    }

    identities
        .remove_identity(auth_state.account_id, path_params.provider)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn unlink_provider_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Unlink a provider")
        .description(
            "Removes a linked OIDC provider from the account. Refused if the provider is the \
             account's only sign-in method.",
        )
        .response_with::<204, (), _>(|res| res.description("Provider unlinked."))
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Returns the authenticated account-identity routes.
///
/// Mounted under the singular `/account/` self-resource (matching `/account/`
/// for the profile), not the plural `/accounts/{accountId}/` collection that
/// addresses other accounts — the identity is always the authenticated caller's.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/account/identities/",
            get_with(list_identities, list_identities_docs),
        )
        .api_route(
            "/account/identities/password/",
            put_with(set_password, set_password_docs)
                .delete_with(delete_password, delete_password_docs),
        )
        // Link (POST) and unlink (DELETE) act on the same provider identity, so
        // they share one URL. Linking begins the OIDC redirect flow (its handler
        // lives in `auth_oidc` beside the shared callback) and returns an
        // authorize URL; unlinking removes the identity.
        .api_route(
            "/account/identities/{provider}/",
            post_with(
                super::auth_oidc::start_link,
                super::auth_oidc::start_link_docs,
            )
            .delete_with(unlink_provider, unlink_provider_docs),
        )
        .with_path_items(|item| item.tag("Identities"))
}
