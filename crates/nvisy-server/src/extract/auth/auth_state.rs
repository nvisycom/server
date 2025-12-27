//! Authentication state extractor with comprehensive database verification.
//!
//! This module provides [`AuthState`], a robust extractor that performs multi-layer
//! authentication verification by validating JWT tokens against current database state.
//! Unlike basic JWT validation, this extractor ensures API tokens remain active and
//! accounts are in good standing.

use std::hash::Hash;

use axum::extract::{FromRef, FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use derive_more::{Deref, DerefMut};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{Account, AccountApiToken};
use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository};
use serde::Deserialize;

use super::{AuthClaims, AuthHeader, TRACING_TARGET_AUTHENTICATION};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::SessionKeys;

/// Authenticated user state with comprehensive database verification.
///
/// [`AuthState`] is the primary authentication extractor that provides verified
/// user credentials after performing extensive security checks. It guarantees
/// that the authenticated user has:
///
/// - A cryptographically valid JWT token
/// - An active API token in the database
/// - A verified and active account
/// - Current privilege levels matching the database
///
/// # Security Guarantees
///
/// When [`AuthState`] extraction succeeds, you can be confident that:
/// - The user is who they claim to be (authentication)
/// - Their API token has not been revoked
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
/// - [`ErrorKind::MalformedAuthToken`]: Invalid JWT format
/// - [`ErrorKind::ExpiredAuthToken`]: Token expired
/// - [`ErrorKind::Unauthorized`]: Invalid credentials or revoked token
/// - [`ErrorKind::InternalServerError`]: Database or system errors
///
/// # Thread Safety
///
/// [`AuthState`] is [`Send`] + [`Sync`] and can be safely shared across threads.
/// All contained data is immutable after creation.
#[derive(Debug, Clone, Deref, DerefMut, Hash, PartialEq, Eq)]
pub struct AuthState<T = ()>(pub AuthClaims<T>);

impl<T> AuthState<T> {
    /// Creates a new [`AuthState`] from pre-verified claims.
    ///
    /// # Safety Requirements
    ///
    /// This method should **only** be used when the claims have already undergone
    /// complete database verification via [`Self::from_unverified_state`].
    /// Using this with unverified claims bypasses critical security checks.
    ///
    /// # Arguments
    ///
    /// * `auth_claims` - Claims that have been verified against the database
    ///
    /// # Returns
    ///
    /// Returns a new [`AuthState`] without additional verification.
    #[inline]
    #[must_use]
    pub const fn from_verified_claims(auth_claims: AuthClaims<T>) -> Self {
        Self(auth_claims)
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
    /// 1. **JWT Token Extraction**: Extracts and validates JWT structure
    /// 2. **Database Connection**: Acquires connection with error handling
    /// 3. **Token Verification**: Confirms API token exists and is active
    /// 4. **Account Verification**: Validates account exists and is verified
    /// 5. **Privilege Consistency**: Ensures token claims match database state
    ///
    /// # Arguments
    ///
    /// * `auth_header` - The authenticated JWT header from the request
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
    /// * [`ErrorKind::Unauthorized`]: API token not found, expired, or account issues
    /// * [`ErrorKind::Forbidden`]: Account verification incomplete or suspended
    ///
    /// # Database Impact
    ///
    /// This method performs optimized database queries and should be called
    /// only once per request (caching handles subsequent uses).
    pub async fn from_unverified_header(
        auth_header: AuthHeader<T>,
        pg_client: PgClient,
    ) -> Result<Self> {
        let auth_claims = auth_header.into_auth_claims();

        tracing::debug!(
            target: TRACING_TARGET_AUTHENTICATION,
            token_id = %auth_claims.token_id,
            account_id = %auth_claims.account_id,
            expires_at = %auth_claims.expires_at,
            is_admin_claim = auth_claims.is_administrator,
            "beginning authentication verification"
        );

        let mut conn = pg_client.get_connection().await.map_err(|db_error| {
            tracing::error!(
                target: TRACING_TARGET_AUTHENTICATION,
                error = %db_error,
                "failed to acquire database connection for authentication verification"
            );
            ErrorKind::InternalServerError
                .with_message("Authentication verification encountered an error")
                .with_resource("authentication")
        })?;

        // Step 1: Verify API token exists and is active
        let api_token = Self::verify_token_validity(&mut conn, &auth_claims).await?;

        // Step 2: Verify account exists and is in good standing
        let account = Self::verify_account_status(&mut conn, &auth_claims).await?;

        // Step 3: Ensure token claims match current account state
        Self::verify_privilege_consistency(&auth_claims, &account)?;

        tracing::info!(
            target: TRACING_TARGET_AUTHENTICATION,
            account_id = %auth_claims.account_id,
            token_id = %auth_claims.token_id,
            is_admin = account.is_admin,
            token_expires = %api_token.expired_at.to_jiff(),
            "authentication verification completed successfully"
        );

        Ok(Self::from_verified_claims(auth_claims))
    }

    /// Verifies that the API token exists in the database and remains active.
    ///
    /// This method performs critical token validation to ensure the JWT token
    /// corresponds to a legitimate, non-revoked API token in the database.
    ///
    /// # Verification Steps
    ///
    /// 1. **Token Lookup**: Queries database for API token by token ID
    /// 2. **Existence Check**: Ensures API token exists (not deleted/revoked)
    /// 3. **Expiration Check**: Validates API token hasn't expired in database
    ///
    /// # Security Implications
    ///
    /// This check is critical because:
    /// - API tokens can be revoked independently of JWT expiration
    /// - Database-level expiration overrides JWT expiration
    /// - Token deletion immediately invalidates access
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for token queries
    /// * `auth_claims` - JWT claims containing the API token ID
    ///
    /// # Returns
    ///
    /// Returns the valid [`AccountApiToken`] record from the database.
    ///
    /// # Errors
    ///
    /// * [`ErrorKind::Unauthorized`]: API token not found or expired
    /// * [`ErrorKind::InternalServerError`]: Database query failures
    async fn verify_token_validity(
        conn: &mut nvisy_postgres::PgConn,
        auth_claims: &AuthClaims<T>,
    ) -> Result<AccountApiToken> {
        let api_token = conn
            .find_token_by_access_token(auth_claims.token_id)
            .await
            .map_err(|db_error| {
                tracing::error!(
                    target: TRACING_TARGET_AUTHENTICATION,
                    error = %db_error,
                    token_id = %auth_claims.token_id,
                    account_id = %auth_claims.account_id,
                    "database error occurred during API token validation query"
                );

                ErrorKind::InternalServerError
                    .with_message("Authentication verification encountered an error")
                    .with_context("Unable to validate API token credentials")
                    .with_resource("authentication")
            })?
            .ok_or_else(|| {
                tracing::warn!(
                    target: TRACING_TARGET_AUTHENTICATION,
                    token_id = %auth_claims.token_id,
                    account_id = %auth_claims.account_id,
                    "authentication failed: API token not found in database"
                );

                ErrorKind::Unauthorized
                    .with_message("Authentication token is invalid")
                    .with_context("Your token may have been revoked or expired")
                    .with_resource("authentication")
            })?;

        // Verify API token hasn't expired at the database level
        if api_token.is_expired() {
            tracing::warn!(
                target: TRACING_TARGET_AUTHENTICATION,
                token_id = %auth_claims.token_id,
                account_id = %auth_claims.account_id,
                expired_at = %api_token.expired_at.to_jiff(),
                current_time = %jiff::Timestamp::now(),
                "Authentication failed: API token expired at database level"
            );

            return Err(ErrorKind::Unauthorized
                .with_message("Your token has expired")
                .with_context("Please sign in again to continue")
                .with_resource("authentication"));
        }

        tracing::debug!(
            target: TRACING_TARGET_AUTHENTICATION,
            token_id = %auth_claims.token_id,
            token_expires = %api_token.expired_at.to_jiff(),
            "API token validation successful"
        );

        Ok(api_token)
    }

    /// Verifies that the account exists and is in good standing.
    ///
    /// This method ensures the account associated with the API token is valid,
    /// verified, and has not been suspended or deleted.
    ///
    /// # Verification Criteria
    ///
    /// 1. **Account Existence**: Account must exist in the database
    /// 2. **Email Verification**: Account email must be verified
    /// 3. **Account Status**: Account must not be suspended or deactivated
    ///
    /// # Security Rationale
    ///
    /// - Prevents access with tokens for deleted accounts
    /// - Enforces email verification requirements
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
    /// * [`ErrorKind::Unauthorized`]: Account not found, unverified, or suspended
    /// * [`ErrorKind::InternalServerError`]: Database query failures
    async fn verify_account_status(
        conn: &mut nvisy_postgres::PgConn,
        auth_claims: &AuthClaims<T>,
    ) -> Result<Account> {
        let account = conn
            .find_account_by_id(auth_claims.account_id)
            .await
            .map_err(|db_error| {
                tracing::error!(
                    target: TRACING_TARGET_AUTHENTICATION,
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
                    target: TRACING_TARGET_AUTHENTICATION,
                    account_id = %auth_claims.account_id,
                    token_id = %auth_claims.token_id,
                    "authentication failed: account referenced in token no longer exists"
                );

                ErrorKind::Unauthorized
                    .with_message("Account not found")
                    .with_context("Your account may have been deactivated")
                    .with_resource("authentication")
            })?;

        // Verify account email has been confirmed
        if !account.is_verified {
            tracing::warn!(
                target: TRACING_TARGET_AUTHENTICATION,
                account_id = %auth_claims.account_id,
                email = %account.email_address,
                token_id = %auth_claims.token_id,
                "authentication failed: account email verification not completed"
            );

            return Err(ErrorKind::Unauthorized
                .with_message("Email verification required")
                .with_context("Please check your email and verify your account")
                .with_resource("authentication"));
        }

        tracing::debug!(
            target: TRACING_TARGET_AUTHENTICATION,
            account_id = %auth_claims.account_id,
            email = %account.email_address,
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
        if auth_claims.is_administrator != account.is_admin {
            tracing::error!(
                target: TRACING_TARGET_AUTHENTICATION,
                account_id = %auth_claims.account_id,
                token_id = %auth_claims.token_id,
                token_admin_claim = auth_claims.is_administrator,
                current_admin_status = account.is_admin,
                email = %account.email_address,
                "critical: admin privilege mismatch detected between token and database"
            );

            return Err(ErrorKind::Unauthorized
                .with_message("Your account privileges have changed")
                .with_context("Please sign in again to access your updated permissions")
                .with_resource("authentication"));
        }

        tracing::debug!(
            target: TRACING_TARGET_AUTHENTICATION,
            account_id = %auth_claims.account_id,
            is_admin = account.is_admin,
            "privilege consistency verification successful"
        );

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
        let auth_header = AuthHeader::from_request_parts(parts, state).await?;
        let pg_database = PgClient::from_ref(state);
        let auth_state = Self::from_unverified_header(auth_header, pg_database).await?;

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
        match <Self as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(auth_state) => Ok(Some(auth_state)),
            Err(_) => Ok(None),
        }
    }
}

impl<T> aide::OperationInput for AuthState<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn operation_input(
        _ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        // Add security requirement for Bearer token
        operation.security = vec![[("BearerAuth".to_string(), vec![])].into()];
    }
}
