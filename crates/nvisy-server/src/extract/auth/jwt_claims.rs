use std::borrow::Cow;

use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use jiff::{Span, Timestamp};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use nvisy_postgres::model::{Account, AccountApiToken};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::auth::TRACING_TARGET_AUTHENTICATION;
use crate::handler::{ErrorKind, Result};

/// JWT claims for authentication tokens.
///
/// This structure contains both RFC 7519 standard JWT claims and nvisy-specific claims.
/// All timestamps use RFC 3339 format for consistency and interoperability.
#[derive(Debug, Clone, Deserialize, Serialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthClaims<T = ()> {
    // Standard (or registered) claims.
    /// Issuer (who created the token).
    #[serde(rename = "iss")]
    issued_by: Cow<'static, str>,
    /// Audience (who the token is intended for).
    #[serde(rename = "aud")]
    audience: Cow<'static, str>,

    // JWT ID (unique identifier for token, useful for revocation).
    #[serde(rename = "jti")]
    pub token_id: Uuid,
    /// Subject ID (unique identifier for associated accound).
    #[serde(rename = "sub")]
    pub account_id: Uuid,

    /// Issued at (as UTC timestamp).
    #[serde(rename = "iat")]
    pub issued_at: Timestamp,
    /// Expiration time (as UTC timestamp).
    #[serde(rename = "exp")]
    pub expires_at: Timestamp,

    // Private (or custom) claims
    #[serde(flatten)]
    pub custom_claims: T,
    /// Is administrator flag.
    #[serde(rename = "cre")]
    pub is_administrator: bool,
}

impl AuthClaims<()> {
    /// Creates a new JWT claims structure from account and session data.
    ///
    /// This method generates claims that are consistent with the database state
    /// at the time of token creation.
    ///
    /// # Arguments
    ///
    /// * `account` - The authenticated account
    /// * `account_session` - The active session for this account
    ///
    /// # Returns
    ///
    /// Returns a new [`AuthClaims`] instance ready for JWT encoding.
    pub fn new(account_model: &Account, account_api_token: &AccountApiToken) -> Self {
        Self::with_custom_claims(account_model, account_api_token, ())
    }
}

impl<T> AuthClaims<T> {
    /// Default JWT audience identifier for authentication tokens.
    const JWT_AUDIENCE: &str = "nvisy:server";
    /// Default JWT issuer identifier for authentication tokens.
    const JWT_ISSUER: &str = "nvisy";
    /// Default threshold for token expiration (5 minutes).
    const SOON_THRESHOLD_MINUTES: i64 = 5;

    /// Creates a new JWT claims structure from account, session data and custom claims.
    ///
    /// This method generates claims that are consistent with the database state
    /// at the time of token creation.
    ///
    /// # Arguments
    ///
    /// * `account` - The authenticated account
    /// * `account_session` - The active session for this account
    /// * `custom_claims` - Custom claims to include in the JWT
    ///
    /// # Returns
    ///
    /// Returns a new [`AuthClaims`] instance ready for JWT encoding.
    pub fn with_custom_claims(
        account_model: &Account,
        account_api_token: &AccountApiToken,
        custom_claims: T,
    ) -> Self {
        Self {
            issued_by: Cow::Borrowed(Self::JWT_ISSUER),
            audience: Cow::Borrowed(Self::JWT_AUDIENCE),
            token_id: account_api_token.access_seq,
            account_id: account_model.id,
            issued_at: account_api_token.issued_at.into(),
            expires_at: account_api_token.expired_at.into(),
            custom_claims,
            is_administrator: account_model.is_admin,
        }
    }

    /// Checks if the token has expired based on current UTC time.
    ///
    /// # Returns
    ///
    /// Returns `true` if the token's expiration time has passed.
    #[inline]
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Timestamp::now()
    }

    /// Checks if the token will expire soon and should be refreshed.
    ///
    /// # Returns
    ///
    /// Returns `true` if the token expires within the configured threshold.
    #[inline]
    #[must_use]
    pub fn expires_soon(&self) -> bool {
        let remaining = self.expires_at - Timestamp::now();
        remaining.get_minutes() < Self::SOON_THRESHOLD_MINUTES
    }

    /// Returns the remaining lifetime of this token.
    ///
    /// # Returns
    ///
    /// The duration until expiration, or zero if already expired.
    #[inline]
    #[must_use]
    pub fn remaining_lifetime(&self) -> Span {
        let remaining = self.expires_at - Timestamp::now();
        if remaining.get_seconds() > 0 {
            remaining
        } else {
            Span::new()
        }
    }
}

impl<T> AuthClaims<T>
where
    T: Clone + Serialize,
{
    /// Encodes the claims into a signed JWT token and creates an Authorization header.
    ///
    /// # Arguments
    ///
    /// * `encoding_key` - The private key for token signing
    ///
    /// # Returns
    ///
    /// Returns a typed Authorization Bearer header ready for HTTP responses.
    ///
    /// # Errors
    ///
    /// Returns errors for JWT encoding failures or invalid token format.
    pub fn into_header(
        self,
        encoding_key: &EncodingKey,
    ) -> Result<TypedHeader<Authorization<Bearer>>> {
        let header = Header::new(Algorithm::EdDSA);
        let jwt_token = encode(&header, &self, encoding_key).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_AUTHENTICATION,
                error = %e,
                account_id = %self.account_id,
                "Failed to encode JWT token"
            );

            ErrorKind::InternalServerError
                .with_message("Authentication token generation failed")
                .with_context("Unable to create session token")
                .with_resource("authentication")
        })?;

        let bearer_auth = Authorization::bearer(&jwt_token).map_err(|_| {
            tracing::error!(
                target: TRACING_TARGET_AUTHENTICATION,
                account_id = %self.account_id,
                "Generated JWT token has invalid format for Authorization header"
            );

            ErrorKind::InternalServerError
                .with_message("Authentication header creation failed")
                .with_context("Generated token format is invalid")
                .with_resource("authentication")
        })?;

        Ok(TypedHeader(bearer_auth))
    }
}

impl<T> AuthClaims<T>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    /// Parses and validates a JWT token from an Authorization header.
    ///
    /// This method performs comprehensive validation including:
    /// - Signature verification using EdDSA
    /// - Standard JWT claims validation (iss, aud, exp, etc.)
    /// - Application-specific claim presence
    /// - Expiration checking with detailed logging
    ///
    /// # Arguments
    ///
    /// * `auth_header` - The Authorization Bearer header
    /// * `decoding_key` - The public key for signature verification
    ///
    /// # Returns
    ///
    /// Returns validated [`AuthClaims`] on success.
    ///
    /// # Errors
    ///
    /// Returns various authentication errors for invalid tokens.
    pub fn from_header(
        auth_header: TypedHeader<Authorization<Bearer>>,
        decoding_key: &DecodingKey,
    ) -> Result<Self> {
        let auth_token = auth_header.token();

        // Configure comprehensive JWT validation
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = true;
        validation.validate_nbf = false; // Not Before claim not used
        validation.validate_aud = true;
        validation.set_audience(&[Self::JWT_AUDIENCE]);
        validation.set_issuer(&[Self::JWT_ISSUER]);
        validation
            .set_required_spec_claims(&["iss", "aud", "jti", "sub", "iat", "exp", "pol", "cre"]);

        tracing::debug!(
            target: TRACING_TARGET_AUTHENTICATION,
            audience = Self::JWT_AUDIENCE,
            issuer = Self::JWT_ISSUER,
            "Validating JWT token with strict security settings"
        );

        let token_data = decode::<Self>(auth_token, decoding_key, &validation)?;
        let claims = token_data.claims;

        // Double-check expiration for security
        if claims.is_expired() {
            tracing::warn!(
                target: TRACING_TARGET_AUTHENTICATION,
                token_id = %claims.token_id,
                account_id = %claims.account_id,
                expired_at = %claims.expires_at,
                "JWT token validation failed: token expired"
            );

            return Err(ErrorKind::Unauthorized
                .with_message("Authentication session has expired")
                .with_context("Please sign in again to continue")
                .with_resource("authentication"));
        }

        tracing::debug!(
            target: TRACING_TARGET_AUTHENTICATION,
            token_id = %claims.token_id,
            account_id = %claims.account_id,
            is_admin = claims.is_administrator,
            expires_soon = claims.expires_soon(),
            remaining = ?claims.remaining_lifetime(),
            "JWT token validation completed successfully"
        );

        Ok(claims)
    }
}
