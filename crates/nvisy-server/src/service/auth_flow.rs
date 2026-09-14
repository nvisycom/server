//! Password sign-in flow orchestration: login, signup, and logout.
//!
//! The cross-cutting authentication logic a handler would otherwise inline —
//! account lookup with constant-time anti-enumeration, the account-status gating,
//! the signup account+identity transaction, and session revocation — factored out
//! of [`authentication`](crate::handler::authentication) so the handler stays thin
//! transport (cookies, request/response shaping).
//!
//! Session *minting* itself lives in [`AuthIssuer`](crate::service::AuthIssuer);
//! this service orchestrates the surrounding rules and delegates the mint. It
//! returns the signed JWT for the handler to wrap in a session cookie, and never
//! touches cookies itself.

use nvisy_postgres::model::{NewAccount, NewAccountIdentity};
use nvisy_postgres::query::{
    AccountApiTokenRepository, AccountIdentityRepository, AccountRepository,
};
use nvisy_postgres::types::IdentityProvider;
use nvisy_postgres::{AsyncConnection, Error as PgError, PgClient};

use crate::extract::SecurityContext;
use crate::handler::request::{Login, Signup};
use crate::handler::utility::build_password_user_inputs;
use crate::response::{ErrorKind, Result};
use crate::service::{AuthIssuer, PasswordService};

/// Tracing target for authentication-flow operations.
const TRACING_TARGET: &str = "nvisy_server::service::auth_flow";

/// Tracing target for authentication cleanup operations.
const TRACING_TARGET_CLEANUP: &str = "nvisy_server::service::auth_flow::cleanup";

/// Orchestrates password login, signup, and logout.
///
/// Holds the Postgres client (own-connection-per-call), the password service (to
/// strength-check, hash, and verify), and the auth issuer (to mint the session).
/// Resolved per request from [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct SignInService {
    postgres: PgClient,
    password: PasswordService,
    issuer: AuthIssuer,
}

impl SignInService {
    /// Creates a [`SignInService`] over its clients.
    pub fn new(postgres: PgClient, password: PasswordService, issuer: AuthIssuer) -> Self {
        Self {
            postgres,
            password,
            issuer,
        }
    }

    /// Authenticates a password login and mints a browser session, returning the
    /// signed session JWT.
    ///
    /// The password hash lives on the account's password identity, not the account,
    /// so an OIDC-only account (no password identity) cannot log in this way.
    ///
    /// A hash verification is performed on *every* attempt — a dummy verify when
    /// there is no account or no password identity — to keep the response time
    /// constant and prevent account enumeration. Do not short-circuit before the
    /// verify.
    pub async fn login(&self, request: Login, security: SecurityContext) -> Result<String> {
        let mut conn = self.postgres.get_connection().await?;
        let account = conn.find_account_by_identifier(&request.identifier).await?;

        // The password hash lives on the account's password identity, not the
        // account. An account with no password identity (OIDC-only) cannot log in
        // by password.
        let password_secret = match &account {
            Some(acc) => conn
                .find_account_identity(acc.id, IdentityProvider::Password)
                .await?
                .and_then(|identity| identity.secret),
            None => None,
        };

        // Always perform a hash verification (a dummy when there is no account or
        // no password identity) to keep timing constant and prevent account
        // enumeration.
        let password_valid = match &password_secret {
            Some(secret) => self.password.verify(&request.password, secret).is_ok(),
            None => self.password.verify_dummy(&request.password),
        };

        let account = match account {
            None => {
                tracing::warn!(target: TRACING_TARGET, reason = "account_not_found", "Login failed");
                return Err(ErrorKind::Unauthorized.with_message("Invalid credentials"));
            }
            Some(_) if !password_valid => {
                tracing::warn!(target: TRACING_TARGET, reason = "invalid_password", "Login failed");
                return Err(ErrorKind::Unauthorized.with_message("Invalid credentials"));
            }
            Some(acc) if acc.is_suspended() => {
                tracing::warn!(target: TRACING_TARGET, reason = "account_suspended", "Login failed");
                return Err(ErrorKind::Forbidden.with_message("Account is suspended"));
            }
            Some(acc) if acc.is_deleted() => {
                tracing::warn!(target: TRACING_TARGET, reason = "account_deleted", "Login failed");
                return Err(ErrorKind::Forbidden.with_message("Account has been deleted"));
            }
            Some(acc) => acc,
        };

        let jwt = self
            .issuer
            .issue_web_session(&mut conn, &account, request.remember_me, security)
            .await?;

        Ok(jwt)
    }

    /// Creates a new account with a password identity and mints a browser session,
    /// returning the signed session JWT.
    ///
    /// The account and its password identity are created together in one
    /// transaction: an account must never exist without a way to authenticate, and
    /// the password hash lives on the identity, not the account.
    pub async fn signup(&self, request: Signup, security: SecurityContext) -> Result<String> {
        // Strength-check and hash before touching the database; bind the check to
        // the account's own fields so a password derived from the username/email is
        // rejected.
        let user_inputs = build_password_user_inputs(
            request.username.as_str(),
            request.display_name.as_deref(),
            &request.email_address,
        );
        let password_hash = self
            .password
            .validate_and_hash(&request.password, &user_inputs)?;

        let mut conn = self.postgres.get_connection().await?;

        // Reject a duplicate email or username before insert; the unique indexes
        // remain the race-safe backstop. The response deliberately does not say
        // *which* field collided: this is the unauthenticated signup path, so a
        // field-specific message would let a caller probe whether a given email is
        // registered. The specific field is logged (server-side) for support.
        let email_taken = conn.email_exists(&request.email_address).await?;
        let username_taken = conn.username_exists(&request.username).await?;
        if email_taken || username_taken {
            tracing::warn!(
                target: TRACING_TARGET,
                email_taken,
                username_taken,
                "Signup failed: email or handle already in use"
            );
            return Err(ErrorKind::Conflict.with_message("That email or handle is already in use"));
        }

        let new_account = NewAccount {
            username: request.username,
            display_name: request.display_name,
            email_address: request.email_address,
            avatar_url: None,
            timezone: None,
            locale: None,
        };

        // Create the account and its password identity together: an account must
        // never exist without a way to authenticate, and the password hash lives on
        // the identity, not the account.
        let account = conn
            .transaction(async |conn| {
                let account = conn.create_account(new_account).await?;
                conn.create_account_identity(NewAccountIdentity::password(
                    account.id,
                    password_hash,
                ))
                .await?;
                Ok::<_, PgError>(account)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, account_id = %account.id, "Account created");

        let jwt = self
            .issuer
            .issue_web_session(&mut conn, &account, request.remember_me, security)
            .await?;

        Ok(jwt)
    }

    /// Revokes the session token `token_id` (logout), returning whether a live
    /// token was found and deleted.
    ///
    /// Revocation is authoritative server-side: the token row is the session
    /// authority, so deleting it ends the session regardless of any client cookie.
    /// A caller logging out on a token that no longer exists is still a success
    /// (there is nothing to revoke); the handler clears the browser cookies in
    /// either case.
    ///
    /// Also opportunistically cleans up this connection's account's expired
    /// sessions — best-effort, so a cleanup failure is only logged.
    pub async fn logout(&self, token_id: uuid::Uuid) -> Result<bool> {
        let mut conn = self.postgres.get_connection().await?;

        // Verify the API token exists before attempting to delete, so the caller
        // learns whether there was a live session to revoke.
        let token_exists = conn.find_account_api_token_by_id(token_id).await?.is_some();

        if !token_exists {
            tracing::warn!(target: TRACING_TARGET, "Logout attempted on non-existent token");
            return Ok(false);
        }

        // Delete the API token (revocation: the row is the session authority).
        let deleted = conn.delete_account_api_token(token_id).await?;
        if deleted {
            tracing::info!(target: TRACING_TARGET, "Logout successful");
        } else {
            tracing::warn!(target: TRACING_TARGET, "Logout completed but token was not found");
        }

        // Opportunistically clean up expired sessions. Run inline on this
        // connection so a pooled slot is not pinned to a detached task; this is
        // best-effort, so a failure is only logged.
        if let Err(e) = conn.cleanup_expired_account_api_tokens().await {
            tracing::debug!(
                target: TRACING_TARGET_CLEANUP,
                error = %e,
                "Failed to cleanup expired sessions during logout"
            );
        }

        Ok(deleted)
    }
}
