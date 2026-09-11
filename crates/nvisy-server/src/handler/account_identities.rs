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
use nvisy_nats::NatsClient;
use nvisy_postgres::query::{AccountIdentityRepository, AccountRepository, DeleteIdentityOutcome};
use nvisy_postgres::types::IdentityProvider;
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};

use super::consume_reauth_proof;
use crate::extract::{AuthState, Json, Path, ValidateJson};
use crate::handler::request::{IdentityPathParams, SetPassword};
use crate::handler::response::AccountIdentities;
use crate::handler::utility::build_password_user_inputs;
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{PasswordService, ServiceState};

/// Tracing target for identity operations.
const TRACING_TARGET: &str = "nvisy_server::handler::identities";

/// Lists the authenticated account's sign-in methods.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn list_identities(
    State(pg_client): State<PgClient>,
    auth_state: AuthState,
) -> Result<Json<AccountIdentities>> {
    tracing::debug!(target: TRACING_TARGET, "Listing account identities");

    let mut conn = pg_client.get_connection().await?;
    let identities = conn
        .list_account_identities(auth_state.account_id)
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
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn set_password(
    State(pg_client): State<PgClient>,
    State(nats): State<NatsClient>,
    State(password): State<PasswordService>,
    auth_state: AuthState,
    ValidateJson(request): ValidateJson<SetPassword>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Setting account password");

    let account_id = auth_state.account_id;
    let mut conn = pg_client.get_connection().await?;
    let account = conn.find_account_by_id(account_id).await?.ok_or_else(|| {
        ErrorKind::NotFound
            .with_message("Account not found")
            .with_resource("account")
    })?;

    let current_secret = conn
        .find_account_identity(account_id, IdentityProvider::Password)
        .await?
        .and_then(|identity| identity.secret);

    match &current_secret {
        // Changing an existing password requires the current password, so a
        // hijacked session or CSRF cannot silently reset it (and lock out the real
        // owner).
        Some(secret) => {
            let verified = request
                .current_password
                .as_deref()
                .is_some_and(|current| password.verify(current, secret).is_ok());
            if !verified {
                tracing::warn!(target: TRACING_TARGET, "Password change failed: current password incorrect");
                return Err(ErrorKind::Unauthorized
                    .with_message("Current password is incorrect")
                    .with_resource("account"));
            }
        }
        // Setting a *first* password creates a new, durable credential, so a live
        // session is not enough — a merely-stolen session could otherwise plant a
        // backdoor. Require a fresh step-up re-authentication proof (from the OIDC
        // reauth endpoint), consumed single-use.
        None => {
            let proof = request.reauth_proof.as_deref().ok_or_else(|| {
                ErrorKind::Unauthorized
                    .with_message("Re-authentication required to set a password")
                    .with_resource("account")
            })?;
            consume_reauth_proof(&nats, account_id, proof).await?;
        }
    }

    // Bind the strength check to the account's own fields so a password derived
    // from the username/email is rejected.
    let user_inputs = build_password_user_inputs(
        account.username.as_str(),
        account.display_name.as_deref(),
        &account.email_address,
    );
    let secret = password.validate_and_hash(&request.new_password, &user_inputs)?;

    // Upsert the password identity and stamp `password_changed_at` together, so a
    // partial failure never leaves the two out of sync.
    conn.transaction(async |conn| {
        conn.upsert_password_secret(account_id, secret).await?;
        conn.update_account(
            account_id,
            nvisy_postgres::model::UpdateAccount {
                password_changed_at: Some(jiff::Timestamp::now().into()),
                ..Default::default()
            },
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

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
    State(pg_client): State<PgClient>,
    auth_state: AuthState,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Removing account password");
    let mut conn = pg_client.get_connection().await?;
    remove_identity(&mut conn, auth_state.account_id, IdentityProvider::Password).await
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
    State(pg_client): State<PgClient>,
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

    let mut conn = pg_client.get_connection().await?;
    remove_identity(&mut conn, auth_state.account_id, path_params.provider).await
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

/// Removes an identity, mapping the last-identity and not-found outcomes to
/// their responses. Shared by the password and provider deletes.
async fn remove_identity(
    conn: &mut PgConn,
    account_id: uuid::Uuid,
    provider: IdentityProvider,
) -> Result<StatusCode> {
    match conn.delete_account_identity(account_id, provider).await? {
        DeleteIdentityOutcome::Deleted => {
            tracing::info!(target: TRACING_TARGET, provider = ?provider, "Identity removed");
            Ok(StatusCode::NO_CONTENT)
        }
        DeleteIdentityOutcome::LastIdentityKept => Err(ErrorKind::Conflict
            .with_message("Cannot remove your only sign-in method; add another first")
            .with_resource("account_identity")),
        DeleteIdentityOutcome::NotFound => Err(ErrorKind::NotFound
            .with_message("No such sign-in method on this account")
            .with_resource("account_identity")),
    }
}

/// Returns the authenticated account-identity routes.
///
/// Mounted under the singular `/account/` self-resource (matching `/account/`
/// for the profile), not the plural `/accounts/{username}/` collection that
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
