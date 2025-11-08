//! JWT authentication header extraction and generation.
//!
//! This module provides JWT token handling for HTTP Authorization headers.
//! It supports both extracting tokens from incoming requests and generating
//! tokens for outgoing responses.
//!
//! # Features
//!
//! - **JWT Validation**: Full JWT token validation with signature verification
//! - **Header Extraction**: Automatic extraction from Authorization Bearer headers
//! - **Response Generation**: Automatic generation of Authorization headers
//! - **Caching**: Request-scoped caching to avoid repeated parsing
//! - **Security**: Comprehensive validation including expiration and issuer checks
//!
//! # Usage
//!
//! As an extractor:
//! ```rust,ignore
//! async fn handler(auth_header: AuthHeader) -> Result<impl IntoResponse> {
//!     let claims = auth_header.as_auth_claims();
//!     // Use the claims...
//! }
//! ```
//!
//! As a response:
//! ```rust,ignore
//! async fn login() -> AuthHeader {
//!     AuthHeader::new(claims, keys)
//! }
//! ```

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use axum_extra::typed_header::TypedHeaderRejectionReason;
use jsonwebtoken::errors::{Error as JwtError, ErrorKind as JwtErrorKind};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use nvisy_postgres::model::{Account, AccountApiToken};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::TRACING_TARGET_AUTHENTICATION;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{SessionKeys};

/// JWT authentication header extractor and response generator.
///
/// This type handles JWT tokens in HTTP Authorization Bearer headers. It can both
/// extract and validate tokens from incoming requests, and generate signed tokens
/// for outgoing responses.
///
/// # Security
///
/// When used as an extractor, the JWT token is validated for:
/// - Signature integrity using the configured keys
/// - Token expiration
/// - Required claims (iss, aud, jti, sub, iat, exp)
/// - Issuer and audience matching
///
/// Note: This extractor only performs JWT validation. For full authentication
/// including database verification, use [`AuthState`] instead.
///
/// # Examples
///
/// Extracting a token from a request:
/// ```rust,ignore
/// use nvisy_server::extract::AuthHeader;
///
/// async fn handler(auth_header: AuthHeader) -> Result<impl IntoResponse> {
///     let claims = auth_header.as_auth_claims();
///     println!("User: {}", claims.account_id);
///     Ok("Success")
/// }
/// ```
///
/// Generating a token for a response:
/// ```rust,ignore
/// async fn login() -> Result<AuthHeader> {
///     let claims = AuthClaims::new(account_id, token_id, is_admin);
///     let keys = AuthKeys::from_env()?;
///     Ok(AuthHeader::new(claims, keys))
/// }
/// ```
///
/// [`AuthState`]: crate::extract::AuthState
#[must_use]
#[derive(Debug, Clone)]
pub struct AuthHeader {
    auth_claims: AuthClaims,
    auth_secret_keys: SessionKeys,
}

impl AuthHeader {
    /// Creates a new authentication header with the given claims and keys.
    ///
    /// # Arguments
    ///
    /// * `claims` - The JWT claims to include in the token
    /// * `keys` - The cryptographic keys for signing the token
    #[inline]
    pub const fn new(claims: AuthClaims, keys: SessionKeys) -> Self {
        Self {
            auth_claims: claims,
            auth_secret_keys: keys,
        }
    }

    /// Returns a reference to the JWT claims.
    #[inline]
    pub const fn as_auth_claims(&self) -> &AuthClaims {
        &self.auth_claims
    }

    /// Consumes this header and returns the JWT claims.
    #[inline]
    pub fn into_auth_claims(self) -> AuthClaims {
        self.auth_claims
    }

    /// Creates an `AuthHeader` from a parsed Authorization header.
    ///
    /// This method validates the JWT token and extracts the claims.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid, expired, or malformed.
    fn from_header(
        authorization_header: TypedHeader<Authorization<Bearer>>,
        auth_secret_keys: SessionKeys,
    ) -> Result<Self> {
        let decoding_key = auth_secret_keys.decoding_key();
        let auth_claims = AuthClaims::from_header(authorization_header, decoding_key)?;
        Ok(Self::new(auth_claims, auth_secret_keys))
    }

    /// Converts this header into an HTTP Authorization header.
    ///
    /// This method signs the JWT token and creates the appropriate header.
    ///
    /// # Errors
    ///
    /// Returns an error if JWT signing fails.
    fn into_header(self) -> Result<TypedHeader<Authorization<Bearer>>> {
        let encoding_key = self.auth_secret_keys.encoding_key();
        self.auth_claims.into_header(encoding_key)
    }
}

impl<S> FromRequestParts<S> for AuthHeader
where
    S: Sync + Send,
    SessionKeys: FromRef<S>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Return cached header if available to avoid re-parsing
        if let Some(auth_header) = parts.extensions.get::<Self>() {
            return Ok(auth_header.clone());
        }

        // Extract Bearer token from Authorization header
        type AuthBearerHeader = TypedHeader<Authorization<Bearer>>;
        let auth_keys = SessionKeys::from_ref(state);

        match AuthBearerHeader::from_request_parts(parts, state).await {
            Ok(bearer_header) => {
                let auth_header = Self::from_header(bearer_header, auth_keys)?;
                // Cache for subsequent extractors in the same request
                parts.extensions.insert(auth_header.clone());
                Ok(auth_header)
            }
            Err(rejection) => {
                let error = match rejection.reason() {
                    TypedHeaderRejectionReason::Missing => ErrorKind::MissingAuthToken
                        .with_message("Authentication required")
                        .with_context("Missing Authorization header with Bearer token")
                        .with_resource("authentication"),
                    TypedHeaderRejectionReason::Error(_) => ErrorKind::MalformedAuthToken
                        .with_message("Invalid token format")
                        .with_context("Authorization header must contain a valid Bearer token")
                        .with_resource("authentication"),
                    _ => ErrorKind::InternalServerError
                        .with_message("Authentication processing failed")
                        .with_context("Unexpected error during header extraction")
                        .with_resource("authentication"),
                };
                Err(error)
            }
        }
    }
}

impl IntoResponseParts for AuthHeader {
    type Error = Error<'static>;

    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        // .into_response_parts() for a TypedHeader is infallible
        self.into_header()
            .map(|h| h.into_response_parts(res).unwrap())
    }
}

impl IntoResponse for AuthHeader {
    fn into_response(self) -> Response {
        // .into_response() for a TypedHeader is infallible
        self.into_header().map(|h| h.into_response()).unwrap()
    }
}

/// JWT claims for authentication tokens.
///
/// This structure contains both RFC 7519 standard JWT claims and nvisy-specific claims.
/// All timestamps use RFC 3339 format for consistency and interoperability.
///
/// # Standard JWT Claims
///
/// | Claim | Field | Description |
/// |-------|-------|-------------|
/// | `iss` | `issued_by` | Token issuer identifier |
/// | `aud` | `audience` | Token audience identifier |
/// | `jti` | `token_id` | Unique session token identifier |
/// | `sub` | `account_id` | Account ID this token represents |
/// | `iat` | `issued_at` | Token creation timestamp |
/// | `exp` | `expired_at` | Token expiration timestamp |
///
/// # Application-Specific Claims
///
/// | Claim | Field | Description |
/// |-------|-------|-------------|
/// | `pol` | `regional_policy` | Data handling policy |
/// | `cre` | `is_administrator` | Global admin privileges |
///
/// # Security Considerations
///
/// - All tokens use EdDSA (Ed25519) signatures
/// - Expiration is enforced at both JWT and database levels
/// - Admin status is verified against current database state
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthClaims {
    // Standard (or registered) claims.
    /// Issuer (who created the token).
    #[serde(rename = "iss")]
    issued_by: String,
    /// Audience (who the token is intended for).
    #[serde(rename = "aud")]
    audience: String,

    // JWT ID (unique identifier for token, useful for revocation).
    #[serde(rename = "jti")]
    pub token_id: Uuid,
    /// Subject ID (unique identifier for associated accound).
    #[serde(rename = "sub")]
    pub account_id: Uuid,

    /// Issued at (as UTC timestamp).
    #[serde(rename = "iat")]
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: OffsetDateTime,
    /// Expiration time (as UTC timestamp).
    #[serde(rename = "exp")]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,

    // Private (or custom) claims
    /// Is administrator flag.
    #[serde(rename = "cre")]
    pub is_administrator: bool,
}

impl AuthClaims {
    /// Default JWT audience identifier for authentication tokens.
    const JWT_AUDIENCE: &str = "nvisy:server";
    /// Default JWT issuer identifier for authentication tokens.
    const JWT_ISSUER: &str = "nvisy";
    /// Default threshold for token expiration.
    const SOON_THRESHOLD: Duration = Duration::minutes(5);

    /// Creates a new JWT claims structure from account and session data.
    ///
    /// This method generates claims that are consistent with the database state
    /// at the time of token creation.
    ///
    /// # Arguments
    ///
    /// * `account` - The authenticated account
    /// * `account_session` - The active session for this account
    /// * `regional_policy` - Data handling policy for this user
    ///
    /// # Returns
    ///
    /// Returns a new [`AuthClaims`] instance ready for JWT encoding.
    pub fn new(
        account_model: Account,
        account_api_token: AccountApiToken,
    ) -> Self {
        Self {
            issued_by: Self::JWT_ISSUER.to_owned(),
            audience: Self::JWT_AUDIENCE.to_owned(),
            token_id: account_api_token.access_seq,
            account_id: account_model.id,
            issued_at: account_api_token.issued_at,
            expires_at: account_api_token.expired_at,
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
        self.expires_at <= OffsetDateTime::now_utc()
    }

    /// Checks if the token will expire soon and should be refreshed.
    ///
    /// # Returns
    ///
    /// Returns `true` if the token expires within the configured threshold.
    #[inline]
    #[must_use]
    pub fn expires_soon(&self) -> bool {
        self.expires_at - OffsetDateTime::now_utc() < Self::SOON_THRESHOLD
    }

    /// Returns the remaining lifetime of this token.
    ///
    /// # Returns
    ///
    /// The duration until expiration, or zero if already expired.
    #[inline]
    #[must_use]
    pub fn remaining_lifetime(&self) -> Duration {
        let remaining = self.expires_at - OffsetDateTime::now_utc();
        if remaining.is_positive() {
            remaining
        } else {
            Duration::ZERO
        }
    }

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
    fn from_header(
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
                .with_context("Please sign in again to continue"));
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
    fn into_header(self, encoding_key: &EncodingKey) -> Result<TypedHeader<Authorization<Bearer>>> {
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
        })?;

        Ok(TypedHeader(bearer_auth))
    }
}

impl From<JwtError> for Error<'static> {
    fn from(error: JwtError) -> Self {
        match error.kind() {
            JwtErrorKind::ExpiredSignature => ErrorKind::Unauthorized
                .with_message("Your session has expired")
                .with_context("Please sign in again to continue"),
            JwtErrorKind::InvalidToken => ErrorKind::MalformedAuthToken
                .with_message("Authentication token is invalid")
                .with_context("The provided token format is unrecognized"),
            JwtErrorKind::InvalidSignature => ErrorKind::Unauthorized
                .with_message("Authentication token verification failed")
                .with_context("Token signature could not be verified"),
            JwtErrorKind::InvalidAlgorithm => ErrorKind::MalformedAuthToken
                .with_message("Authentication token uses unsupported format")
                .with_context("Token was signed with an incompatible algorithm"),
            JwtErrorKind::InvalidAudience => ErrorKind::Unauthorized
                .with_message("Authentication token is not valid for this service")
                .with_context("Token was issued for a different application"),
            JwtErrorKind::InvalidIssuer => ErrorKind::Unauthorized
                .with_message("Authentication token is from an untrusted source")
                .with_context("Token was not issued by this authentication system"),
            JwtErrorKind::MissingRequiredClaim(claim) => ErrorKind::MalformedAuthToken
                .with_message("Authentication token is incomplete")
                .with_context(format!("Token is missing required field: {}", claim)),
            JwtErrorKind::Base64(_) => ErrorKind::MalformedAuthToken
                .with_message("Authentication token format is corrupted")
                .with_context("Token contains invalid base64 encoding"),
            JwtErrorKind::Json(_) => ErrorKind::MalformedAuthToken
                .with_message("Authentication token structure is invalid")
                .with_context("Token payload contains malformed data"),
            JwtErrorKind::InvalidKeyFormat => ErrorKind::MalformedAuthToken
                .with_message("Authentication token encoding is invalid")
                .with_context("Token contains invalid key format"),
            JwtErrorKind::InvalidEcdsaKey => ErrorKind::InternalServerError
                .with_message("Authentication verification encountered an error")
                .with_context("Cryptographic validation failed"),
            _ => ErrorKind::InternalServerError
                .with_message("Authentication processing failed")
                .with_context("An unexpected error occurred during token validation"),
        }
    }
}
