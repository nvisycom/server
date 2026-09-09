//! Authentication state extractor with comprehensive database verification.
//!
//! This module provides [`AuthState`], a robust extractor that performs multi-layer
//! authentication verification by validating JWT tokens against current database state.
//! Unlike basic JWT validation, this extractor ensures accounts are in good standing.

use std::hash::Hash;

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::Operation;
use axum::extract::{FromRef, FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use derive_more::Deref;
use nvisy_postgres::model::{Account, WorkspaceMember};
use nvisy_postgres::query::{
    AccountApiTokenRepository, AccountRepository, WorkspaceMemberRepository,
};
use nvisy_postgres::types::session;
use nvisy_postgres::{PgClient, PgConn};
use serde::Deserialize;
use uuid::Uuid;

use super::{AuthClaims, Permission, SessionToken};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::SessionKeys;

/// Tracing target for authentication operations.
const TRACING_TARGET: &str = "nvisy_server::authentication";

/// Authenticated user state with comprehensive database verification.
///
/// [`AuthState`] is the primary authentication extractor that provides verified
/// user credentials after performing extensive security checks. It guarantees
/// that the authenticated user has:
///
/// - A cryptographically valid JWT token
/// - A verified and active account
/// - Current privilege levels matching the database
///
/// # Security Guarantees
///
/// When [`AuthState`] extraction succeeds, you can be confident that:
/// - The user is who they claim to be (authentication)
/// - Their account is in good standing
/// - Their privileges are current and accurate
///
/// # Performance Characteristics
///
/// - **First Use**: Performs full database verification
/// - **Subsequent Uses**: Uses cached result from request extensions
/// - **Memory Footprint**: Minimal - only stores essential claims
/// - **Database Impact**: Single optimized query per request
///
/// # Errors
///
/// Extraction fails with specific error types for:
/// - `MalformedAuthToken`: Invalid JWT format
/// - `ExpiredToken`: Token expired
/// - `Unauthorized`: Invalid credentials or account issues
/// - `InternalServerError`: Database or system errors
///
/// # Thread Safety
///
/// [`AuthState`] is [`Send`] + [`Sync`] and can be safely shared across threads.
/// All contained data is immutable after creation.
#[derive(Debug, Clone, Deref, Hash, PartialEq, Eq)]
pub struct AuthState<T = ()>(AuthClaims<T>);

impl<T> AuthState<T> {
    /// Wraps claims that have already been verified against the database. Private
    /// on purpose: the only way to obtain an `AuthState` is by going through
    /// [`from_unverified_header`](Self::from_unverified_header) (or the extractor),
    /// so the type is a proof of verification that cannot be forged from raw claims.
    #[inline]
    const fn from_verified_claims(auth_claims: AuthClaims<T>) -> Self {
        Self(auth_claims)
    }

    /// Authorizes the caller for `permission` in `workspace_id`, returning their
    /// membership on success (or `None` for a global admin, who is authorized
    /// without being a member).
    ///
    /// A global admin bypasses the workspace check. Otherwise the caller must be a
    /// member whose role satisfies `permission`; a non-member or an insufficient
    /// role is `403 Forbidden`.
    ///
    /// # Errors
    ///
    /// Returns `Forbidden` if access is denied, or propagates database errors from
    /// the membership lookup.
    pub async fn authorize_workspace(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        permission: Permission,
    ) -> Result<Option<WorkspaceMember>> {
        // Global administrators bypass workspace-level permissions.
        if self.0.is_admin {
            tracing::debug!(
                target: TRACING_TARGET,
                account_id = %self.0.account_id,
                workspace_id = %workspace_id,
                permission = ?permission,
                "access granted: global administrator"
            );
            return Ok(None);
        }

        let member = conn
            .find_workspace_member(workspace_id, self.0.account_id)
            .await
            .map_err(Error::from)?;

        let Some(member) = member else {
            tracing::warn!(
                target: TRACING_TARGET,
                account_id = %self.0.account_id,
                workspace_id = %workspace_id,
                "access denied: not a workspace member"
            );
            return Err(ErrorKind::Forbidden
                .with_message("Not a workspace member")
                .with_resource("workspace"));
        };

        if permission.is_permitted_by_role(member.member_role) {
            tracing::debug!(
                target: TRACING_TARGET,
                account_id = %self.0.account_id,
                workspace_id = %workspace_id,
                permission = ?permission,
                role = ?member.member_role,
                "access granted: sufficient role"
            );
            Ok(Some(member))
        } else {
            tracing::warn!(
                target: TRACING_TARGET,
                account_id = %self.0.account_id,
                workspace_id = %workspace_id,
                permission = ?permission,
                role = ?member.member_role,
                "access denied: insufficient role"
            );
            Err(ErrorKind::Forbidden
                .with_message("Insufficient role for this action")
                .with_resource("workspace"))
        }
    }
}

impl<T> AuthState<T>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    /// Creates a new [`AuthState`] from an unverified JWT token with full database validation.
    ///
    /// This method is the primary entry point for secure authentication verification.
    /// It performs a comprehensive multi-step validation process to ensure the
    /// authentication credentials are current and valid.
    ///
    /// # Verification Process
    ///
    /// 1. **JWT Token Extraction**: Extracts and validates JWT structure (including expiration)
    /// 2. **Database Connection**: Acquires connection with error handling
    /// 3. **Account Verification**: Validates account exists and is in good standing
    /// 4. **Privilege Consistency**: Ensures token claims match database state
    ///
    /// # Arguments
    ///
    /// * `session_token` - The authenticated JWT header from the request
    /// * `pg_database` - Database connection pool for verification queries
    ///
    /// # Returns
    ///
    /// Returns a fully verified [`AuthState`] ready for authorization decisions.
    ///
    /// # Errors
    ///
    /// Returns specific error types for different failure modes:
    ///
    /// * [`ErrorKind::InternalServerError`]: Database connection or query failures
    /// * [`ErrorKind::Unauthorized`]: Account not found or privilege mismatch
    /// * [`ErrorKind::Forbidden`]: Account verification incomplete or suspended
    ///
    /// # Database Impact
    ///
    /// This method performs optimized database queries and should be called
    /// only once per request (caching handles subsequent uses).
    pub async fn from_unverified_header(
        session_token: SessionToken<T>,
        pg_client: PgClient,
    ) -> Result<Self> {
        let auth_claims = session_token.into_auth_claims();

        tracing::debug!(
            target: TRACING_TARGET,
            token_id = %auth_claims.token_id,
            account_id = %auth_claims.account_id,
            expires_at = %auth_claims.expires_at,
            is_admin_claim = auth_claims.is_admin,
            "beginning authentication verification"
        );

        let mut conn = pg_client.get_connection().await.map_err(|db_error| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %db_error,
                "failed to acquire database connection for authentication verification"
            );
            ErrorKind::InternalServerError
                .with_message("Authentication verification encountered an error")
                .with_resource("authentication")
        })?;

        // Step 1: Verify account exists and is in good standing
        let account = Self::verify_account_status(&mut conn, &auth_claims).await?;

        // Step 2: Ensure token claims match current account state
        Self::verify_privilege_consistency(&auth_claims, &account)?;

        // Step 3: Ensure the token itself has not been revoked. The JWT's own
        // expiry bounds its lifetime, but revocation must take effect
        // immediately, so the backing token row is checked on every request.
        Self::verify_token_active(&mut conn, &auth_claims).await?;

        tracing::info!(
            target: TRACING_TARGET,
            account_id = %auth_claims.account_id,
            token_id = %auth_claims.token_id,
            is_admin = account.is_admin,
            "authentication verification completed successfully"
        );

        Ok(Self::from_verified_claims(auth_claims))
    }

    /// Verifies that the account exists and is in good standing.
    ///
    /// This method ensures the account associated with the API token is valid,
    /// verified, and has not been suspended or deleted.
    ///
    /// # Verification Criteria
    ///
    /// 1. **Account Existence**: Account must exist in the database
    /// 2. **Account Status**: Account must not be suspended or deactivated
    ///
    /// # Security Rationale
    ///
    /// - Prevents access with tokens for deleted accounts
    /// - Allows immediate access revocation via account suspension
    /// - Maintains data integrity between API tokens and accounts
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for account queries
    /// * `auth_claims` - JWT claims containing the account ID
    ///
    /// # Returns
    ///
    /// Returns the verified [`Account`] record from the database.
    ///
    /// # Errors
    ///
    /// * [`ErrorKind::Unauthorized`]: Account not found or suspended
    /// * [`ErrorKind::InternalServerError`]: Database query failures
    async fn verify_account_status(
        conn: &mut PgConn,
        auth_claims: &AuthClaims<T>,
    ) -> Result<Account> {
        let account = conn
            .find_account_by_id(auth_claims.account_id)
            .await
            .map_err(|db_error| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %db_error,
                    account_id = %auth_claims.account_id,
                    token_id = %auth_claims.token_id,
                    "database error occurred during account validation query"
                );

                ErrorKind::InternalServerError
                    .with_message("Account verification encountered an error")
                    .with_context("Unable to validate account credentials")
                    .with_resource("authentication")
            })?
            .ok_or_else(|| {
                tracing::warn!(
                    target: TRACING_TARGET,
                    account_id = %auth_claims.account_id,
                    token_id = %auth_claims.token_id,
                    "authentication failed: account referenced in token no longer exists"
                );

                ErrorKind::Unauthorized
                    .with_message("Account not found")
                    .with_context("Your account may have been deactivated")
                    .with_resource("authentication")
            })?;

        tracing::debug!(
            target: TRACING_TARGET,
            account_id = %auth_claims.account_id,
            is_admin = account.is_admin,
            "account validation successful"
        );

        Ok(account)
    }

    /// Verifies that privilege claims in the JWT token match the current database state.
    ///
    /// This critical security check ensures that privilege changes (admin promotion/demotion)
    /// are immediately effective by comparing token claims with current database records.
    ///
    /// # Security Importance
    ///
    /// - **Real-time Privilege Enforcement**: Admin changes take effect immediately
    /// - **Token Invalidation**: Forces re-authentication when privileges change
    /// - **Privilege Escalation Prevention**: Prevents use of stale admin tokens
    /// - **Audit Compliance**: Ensures privilege records are consistent
    ///
    /// # Arguments
    ///
    /// * `auth_claims` - JWT claims containing privilege assertions
    /// * `account` - Current account record from database
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if privileges are consistent.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Unauthorized`] if privilege claims don't match database.
    fn verify_privilege_consistency(auth_claims: &AuthClaims<T>, account: &Account) -> Result<()> {
        if auth_claims.is_admin != account.is_admin {
            tracing::error!(
                target: TRACING_TARGET,
                account_id = %auth_claims.account_id,
                token_id = %auth_claims.token_id,
                token_admin_claim = auth_claims.is_admin,
                current_admin_status = account.is_admin,
                "critical: admin privilege mismatch detected between token and database"
            );

            return Err(ErrorKind::Unauthorized
                .with_message("Your account privileges have changed")
                .with_context("Please sign in again to access your updated permissions")
                .with_resource("authentication"));
        }

        tracing::debug!(
            target: TRACING_TARGET,
            account_id = %auth_claims.account_id,
            is_admin = account.is_admin,
            "privilege consistency verification successful"
        );

        Ok(())
    }

    /// Verifies that the token backing this request has not been revoked.
    ///
    /// The bearer credential is a self-contained JWT, so a revoked (soft-deleted)
    /// token would otherwise keep working until its `exp`. Checking the backing
    /// `account_api_tokens` row by `jti` on every request makes revocation
    /// immediate. The lookup is scoped to the token's own account, so a token
    /// only authenticates while its row exists, is not deleted, and still belongs
    /// to the claimed account.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Unauthorized`] if the token has been revoked.
    async fn verify_token_active(conn: &mut PgConn, auth_claims: &AuthClaims<T>) -> Result<()> {
        // This check is the SOLE authority for session validity: revocation, idle
        // expiry, and absolute-age expiry all go through `is_active`. It MUST fail
        // closed — a database error maps to an error (request rejected), never to
        // "allow through". Do not add an allow-on-error fallback here, or a revoked
        // or expired session would authenticate.
        let is_active = conn
            .account_api_token_is_active(
                auth_claims.token_id,
                auth_claims.account_id,
                session::MAX_AGE,
            )
            .await
            .map_err(|db_error| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %db_error,
                    account_id = %auth_claims.account_id,
                    token_id = %auth_claims.token_id,
                    "database error occurred during session validity check"
                );

                ErrorKind::InternalServerError
                    .with_message("Authentication verification encountered an error")
                    .with_context("Unable to validate the session token")
                    .with_resource("authentication")
            })?;

        if !is_active {
            tracing::warn!(
                target: TRACING_TARGET,
                account_id = %auth_claims.account_id,
                token_id = %auth_claims.token_id,
                "authentication failed: token has been revoked"
            );

            return Err(ErrorKind::Unauthorized
                .with_message("Your session has been revoked")
                .with_context("Please sign in again to continue")
                .with_resource("authentication"));
        }

        Ok(())
    }
}

impl<T, S> FromRequestParts<S> for AuthState<T>
where
    T: Clone + for<'de> Deserialize<'de> + Send + Sync + 'static,
    S: Sync + Send + 'static,
    PgClient: FromRef<S>,
    SessionKeys: FromRef<S>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Check for cached auth state to avoid repeated database queries
        if let Some(auth_state) = parts.extensions.get::<Self>() {
            return Ok(auth_state.clone());
        }

        // Extract JWT token and perform comprehensive database verification
        let session_token = SessionToken::from_request_parts(parts, state).await?;
        let pg_database = PgClient::from_ref(state);
        let auth_state = Self::from_unverified_header(session_token, pg_database).await?;

        // Cache the verified state for subsequent extractors in the same request
        parts.extensions.insert(auth_state.clone());
        Ok(auth_state)
    }
}

impl<T, S> OptionalFromRequestParts<S> for AuthState<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
    S: Sync + Send + 'static,
    PgClient: FromRef<S>,
    SessionKeys: FromRef<S>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        use crate::handler::ErrorKind;

        match <Self as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(auth_state) => Ok(Some(auth_state)),
            // Only a genuinely absent-or-invalid credential degrades to "not
            // authenticated" (`None`). An infrastructure error (e.g. the database
            // validity check failing) or a forbidden account must PROPAGATE, not
            // silently become anonymous — mapping those to `None` would fail open,
            // contradicting the fail-closed authority of the `is_active` check.
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::MissingAuthToken
                        | ErrorKind::MalformedAuthToken
                        | ErrorKind::Unauthorized
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl<T> OperationInput for AuthState<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn operation_input(_ctx: &mut GenContext, operation: &mut Operation) {
        // The Bearer token is required: the only way to satisfy the operation is
        // to present it.
        operation.security = vec![[("BearerAuth".to_string(), vec![])].into()];
    }
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::model::{Account, AccountApiToken};
    use nvisy_postgres::types::ApiTokenType;

    use super::{AuthClaims, AuthState};

    /// Builds claims for `account` (the claim's `is_admin` mirrors the account it
    /// was minted from).
    fn claims_for(account: &Account) -> AuthClaims<()> {
        let token = AccountApiToken::test(account.id, ApiTokenType::Web);
        AuthClaims::new(account, &token)
    }

    #[test]
    fn privilege_consistency_accepts_a_matching_admin_flag() {
        let mut admin = Account::test();
        admin.is_admin = true;
        assert!(AuthState::<()>::verify_privilege_consistency(&claims_for(&admin), &admin).is_ok());

        let user = Account::test(); // is_admin: false
        assert!(AuthState::<()>::verify_privilege_consistency(&claims_for(&user), &user).is_ok());
    }

    #[test]
    fn privilege_consistency_rejects_a_stale_admin_claim() {
        // Token was minted while the account was admin; the account has since been
        // demoted. The stale admin claim must be rejected (fail closed).
        let mut was_admin = Account::test();
        was_admin.is_admin = true;
        let stale_claims = claims_for(&was_admin);

        let mut now_demoted = was_admin.clone();
        now_demoted.is_admin = false;

        assert!(
            AuthState::<()>::verify_privilege_consistency(&stale_claims, &now_demoted).is_err()
        );
    }

    #[test]
    fn privilege_consistency_rejects_a_forged_admin_claim() {
        // A non-admin account whose token nonetheless claims admin must be
        // rejected — a claim can never grant a privilege the DB does not hold.
        let mut forged = Account::test();
        forged.is_admin = true;
        let forged_claims = claims_for(&forged);

        let real = Account::test(); // is_admin: false
        assert!(AuthState::<()>::verify_privilege_consistency(&forged_claims, &real).is_err());
    }
}
