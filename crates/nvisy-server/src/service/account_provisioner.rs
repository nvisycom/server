//! Account resolution for OIDC sign-in: return the existing account, link the
//! verified identity to an account that already uses the email, or provision a
//! new one.
//!
//! This is the domain logic behind the OIDC callback — stateless, operating on a
//! connection and the verified provider identity — factored out of the handler so
//! the provisioning/linking rules live in one place and can be reasoned about
//! (and tested) independently of the HTTP flow.

use nvisy_postgres::model::{Account, NewAccount, NewAccountIdentity};
use nvisy_postgres::query::{AccountIdentityRepository, AccountRepository, LinkIdentityOutcome};
use nvisy_postgres::types::{HANDLE_MAX_LENGTH, Handle, IdentityProvider};
use nvisy_postgres::{AsyncConnection, Error as PgError, PgConn};
use uuid::Uuid;

use crate::response::{ErrorKind, Result};
use crate::service::OidcIdentity;

/// Tracing target for account provisioning.
const TRACING_TARGET: &str = "nvisy_server::account_provisioner";

/// How many suffixed handles to try when deriving a unique username on
/// provisioning, before giving up. A collision past this many is implausible
/// (each is a distinct suffix), so exhausting it is a server-side failure.
const MAX_USERNAME_ATTEMPTS: u32 = 100;

/// Resolves, links, and provisions accounts for verified OIDC identities.
///
/// Stateless: every method takes the connection to act on. Resolved per request
/// from [`ServiceState`](crate::service::ServiceState).
#[derive(Clone, Copy, Default)]
pub struct AccountProvisioner;

impl AccountProvisioner {
    /// Resolves the account for a verified OIDC identity, in order of preference:
    ///
    /// 1. **Returning user** — an identity already exists for this `(provider,
    ///    subject)`; reuse its account.
    /// 2. **Link to an existing account** — the provider asserts a *verified*
    ///    email that matches an account (e.g. one created by password signup);
    ///    attach a new OIDC identity to it, so the two sign-in methods share one
    ///    account.
    /// 3. **Provision** — otherwise create a new account and its OIDC identity.
    ///
    /// Linking and provisioning both require a verified email: an unverified
    /// address could be one the signer does not control, so acting on it would let
    /// an attacker attach their provider identity to (or seed) someone else's
    /// account.
    ///
    /// # Errors
    ///
    /// Rejects a missing email, an unverified email on a link/provision, or a
    /// provider slot already taken on the matched account; propagates database
    /// errors.
    pub async fn resolve(
        &self,
        conn: &mut PgConn,
        provider: IdentityProvider,
        identity: OidcIdentity,
    ) -> Result<Account> {
        // 1. Returning user: an identity for this subject already exists.
        if let Some(existing) = conn
            .find_identity_by_subject(provider, &identity.subject)
            .await?
            && let Some(account) = conn.find_account_by_id(existing.account_id).await?
        {
            return Ok(account);
        }

        // A new identity needs the provider-asserted email: to provision, it
        // becomes the account's required primary address; to link, it is the match
        // key.
        let email = identity.email.ok_or_else(|| {
            ErrorKind::BadRequest
                .with_message("Sign-in provider did not return an email address")
                .with_resource("account")
        })?;

        // 2. An account already uses this email.
        if let Some(account) = conn.find_account_by_email(&email).await? {
            // Link only when the provider verified the email: an unverified address
            // could be one the signer does not control, and linking on it would let
            // them attach their provider identity to someone else's account.
            if !identity.email_verified {
                tracing::warn!(
                    target: TRACING_TARGET,
                    account_id = %account.id,
                    provider = ?provider,
                    "Refusing to link OIDC identity: provider did not verify the email",
                );
                return Err(ErrorKind::Conflict
                    .with_message(
                        "An account already uses this email; sign in with your existing method \
                         or verify the email with the provider first",
                    )
                    .with_resource("account"));
            }

            // The matched account may already have a *different* identity for this
            // provider (a different subject). Only one identity per provider is
            // allowed, so linking would trip the unique index; surface a clean
            // conflict instead of a 500.
            if conn
                .find_account_identity(account.id, provider)
                .await?
                .is_some()
            {
                tracing::warn!(
                    target: TRACING_TARGET,
                    account_id = %account.id,
                    provider = ?provider,
                    "Refusing to link OIDC identity: account already has one for this provider",
                );
                return Err(ErrorKind::Conflict
                    .with_message(
                        "An account already uses this email with a different provider account",
                    )
                    .with_resource("account"));
            }

            link_oidc_identity(
                conn,
                NewAccountIdentity::oidc(account.id, provider, identity.subject, Some(email)),
            )
            .await?;
            tracing::info!(
                target: TRACING_TARGET,
                account_id = %account.id,
                provider = ?provider,
                "Linked OIDC identity to existing account",
            );
            return Ok(account);
        }

        // 3. Provision a new account and its OIDC identity together, so an account
        // never exists without a way to authenticate.
        //
        // Only provision on a verified email: the address becomes the new account's
        // primary (and its future match key for step 2), so an unverified one could
        // seed an account under an address the signer does not control.
        if !identity.email_verified {
            tracing::warn!(
                target: TRACING_TARGET,
                provider = ?provider,
                "Refusing to provision account: provider did not verify the email",
            );
            return Err(ErrorKind::BadRequest
                .with_message(
                    "Sign-in provider did not verify your email address; verify it with the \
                     provider and try again",
                )
                .with_resource("account"));
        }

        let username = derive_unique_username(conn, &email).await?;
        let new_account = NewAccount {
            username,
            display_name: None,
            email_address: email.clone(),
            avatar_url: None,
            timezone: None,
            locale: None,
        };

        let account = conn
            .transaction(async |conn| {
                let account = conn.create_account(new_account).await?;
                conn.create_account_identity(NewAccountIdentity::oidc(
                    account.id,
                    provider,
                    identity.subject,
                    Some(email),
                ))
                .await?;
                Ok::<_, PgError>(account)
            })
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            account_id = %account.id,
            provider = ?provider,
            "Provisioned account from OIDC sign-in",
        );

        Ok(account)
    }

    /// Attaches a verified OIDC identity to an already-authenticated account (the
    /// account that started an authenticated link flow).
    ///
    /// Idempotent for the same account: re-linking an identity already on this
    /// account is a no-op. Refuses to move an identity already linked to a
    /// *different* account (its provider subject is unique), so one provider login
    /// cannot be hijacked onto another account.
    ///
    /// # Errors
    ///
    /// Returns a conflict if the identity is linked to a different account, a
    /// not-found if the account no longer exists, or a database error.
    pub async fn link(
        &self,
        conn: &mut PgConn,
        account_id: Uuid,
        provider: IdentityProvider,
        identity: OidcIdentity,
    ) -> Result<Account> {
        if let Some(existing) = conn
            .find_identity_by_subject(provider, &identity.subject)
            .await?
        {
            if existing.account_id == account_id {
                // Already linked to this account: nothing to do.
                return self.load_active(conn, account_id).await;
            }
            tracing::warn!(
                target: TRACING_TARGET,
                account_id = %account_id,
                provider = ?provider,
                "Refusing to link an identity already linked to another account",
            );
            return Err(ErrorKind::Conflict
                .with_message("This provider identity is already linked to another account")
                .with_resource("account_identity"));
        }

        link_oidc_identity(
            conn,
            NewAccountIdentity::oidc(account_id, provider, identity.subject, identity.email),
        )
        .await?;
        tracing::info!(
            target: TRACING_TARGET,
            account_id = %account_id,
            provider = ?provider,
            "Linked OIDC identity to the authenticated account",
        );

        self.load_active(conn, account_id).await
    }

    /// Loads a live account by id, or a not-found error (e.g. the account was
    /// deleted between starting a link flow and its callback).
    ///
    /// # Errors
    ///
    /// Returns not-found if no account has the id, or a database error.
    pub async fn load_active(&self, conn: &mut PgConn, account_id: Uuid) -> Result<Account> {
        conn.find_account_by_id(account_id).await?.ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Account not found")
                .with_resource("account")
        })
    }
}

/// Links an OIDC identity to an existing account, mapping the repository's
/// race-tolerant [`LinkIdentityOutcome`] to the handler result: a successful or
/// already-present link is `Ok`, and a provider slot already taken by a
/// *different* account is a clean 409 rather than a 500.
async fn link_oidc_identity(conn: &mut PgConn, identity: NewAccountIdentity) -> Result<()> {
    match conn.link_oidc_identity(identity).await? {
        LinkIdentityOutcome::Linked | LinkIdentityOutcome::AlreadyLinked => Ok(()),
        LinkIdentityOutcome::ProviderConflict => Err(ErrorKind::Conflict
            .with_message("An account already uses a different provider account")
            .with_resource("account_identity")),
    }
}

/// Derives a unique username for a provisioned account from its email local
/// part, appending a numeric suffix on collision.
async fn derive_unique_username(conn: &mut PgConn, email: &str) -> Result<Handle> {
    let local_part = email.split('@').next().unwrap_or(email);
    // A local part may not slugify to a valid handle (too short, no usable
    // characters); fall back to a stable generated base so provisioning still
    // succeeds.
    let base = Handle::derive(local_part).unwrap_or_else(|| {
        Handle::derive(&format!("user-{}", Uuid::now_v7().simple()))
            .expect("a uuid-based handle is always valid")
    });

    if !conn.username_exists(&base).await? {
        return Ok(base);
    }
    // Widest suffix this loop can append, so we reserve room for the largest
    // `-{suffix}` up front. Without this, a `base` already at the length limit
    // would have its suffix truncated straight back off, and every candidate
    // would collapse to `base` and collide forever.
    let widest_suffix = MAX_USERNAME_ATTEMPTS.to_string().len();
    let reserved = HANDLE_MAX_LENGTH.saturating_sub(1 + widest_suffix);
    let stem = truncate_on_char_boundary(base.as_str(), reserved);
    for suffix in 1..=MAX_USERNAME_ATTEMPTS {
        // The stem already leaves room for the separator and suffix, so the
        // re-derive validates the combined form without truncating the suffix away.
        let candidate_text = format!("{stem}-{suffix}");
        if let Some(candidate) = Handle::derive(&candidate_text)
            && !conn.username_exists(&candidate).await?
        {
            return Ok(candidate);
        }
    }

    Err(ErrorKind::InternalServerError
        .with_message("Could not allocate a username for the new account")
        .with_resource("account"))
}

/// Truncates `value` to at most `max` bytes without splitting a UTF-8 character.
/// A derived [`Handle`] is ASCII, so `max` bytes equal `max` characters here.
fn truncate_on_char_boundary(value: &str, max: usize) -> &str {
    if value.len() <= max {
        return value;
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}
