//! JWT authentication header extraction and generation.
//!
//! This module provides JWT token handling for HTTP Authorization headers.
//! It supports both extracting tokens from incoming requests and generating
//! tokens for outgoing responses.

use std::fmt::Debug;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use axum_extra::typed_header::TypedHeaderRejectionReason;
use jsonwebtoken::errors::{Error as JwtError, ErrorKind as JwtErrorKind};
use serde::{Deserialize, Serialize};

use super::AuthClaims;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::AuthKeys;

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
/// # Notes
///
/// This extractor only performs JWT validation. For full authentication
/// including database verification, use [`AuthState`] instead.
///
/// [`AuthState`]: crate::extract::AuthState
#[must_use]
#[derive(Debug, Clone)]
pub struct AuthHeader<T = ()> {
    auth_claims: AuthClaims<T>,
    auth_secret_keys: AuthKeys,
}

impl<T> AuthHeader<T> {
    /// Creates a new authentication header with the given claims and keys.
    ///
    /// # Arguments
    ///
    /// * `claims` - The JWT claims to include in the token
    /// * `keys` - The cryptographic keys for signing the token
    #[inline]
    pub const fn new(claims: AuthClaims<T>, keys: AuthKeys) -> Self {
        Self {
            auth_claims: claims,
            auth_secret_keys: keys,
        }
    }

    /// Returns a reference to the JWT claims.
    #[inline]
    pub const fn as_auth_claims(&self) -> &AuthClaims<T> {
        &self.auth_claims
    }

    /// Consumes this header and returns the JWT claims.
    #[inline]
    pub fn into_auth_claims(self) -> AuthClaims<T> {
        self.auth_claims
    }
}

impl<T> AuthHeader<T>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    /// Creates an `AuthHeader` from a parsed Authorization header.
    ///
    /// This method validates the JWT token and extracts the claims.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid, expired, or malformed.
    fn from_header(
        authorization_header: TypedHeader<Authorization<Bearer>>,
        auth_secret_keys: AuthKeys,
    ) -> Result<Self> {
        let decoding_key = auth_secret_keys.decoding_key();
        let auth_claims = AuthClaims::from_header(authorization_header, decoding_key)?;
        Ok(Self::new(auth_claims, auth_secret_keys))
    }
}

impl<T> AuthHeader<T>
where
    T: Clone + Serialize,
{
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

impl<T, S> FromRequestParts<S> for AuthHeader<T>
where
    T: Clone + for<'de> Deserialize<'de> + Send + Sync + 'static,
    S: Sync + Send,
    AuthKeys: FromRef<S>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Return cached header if available to avoid re-parsing
        if let Some(auth_header) = parts.extensions.get::<Self>() {
            return Ok(auth_header.clone());
        }

        // Extract Bearer token from Authorization header
        type AuthBearerHeader = TypedHeader<Authorization<Bearer>>;
        let auth_keys = AuthKeys::from_ref(state);

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

impl<T> IntoResponseParts for AuthHeader<T>
where
    T: Clone + Serialize,
{
    type Error = Error<'static>;

    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        // .into_response_parts() for a TypedHeader is infallible
        self.into_header()
            .map(|h| h.into_response_parts(res).unwrap())
    }
}

impl<T> IntoResponse for AuthHeader<T>
where
    T: Clone + Serialize,
{
    fn into_response(self) -> Response {
        // .into_response() for a TypedHeader is infallible
        self.into_header().map(|h| h.into_response()).unwrap()
    }
}

impl From<JwtError> for Error<'static> {
    fn from(error: JwtError) -> Self {
        let error = match error.kind() {
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
        };

        error.with_resource("authentication")
    }
}

// Aide OpenAPI support - AuthHeader generates a response header
// Note: For now, we don't implement OperationOutput for AuthHeader since it's only used
// in responses where the Authorization header is added manually via TypedHeader.
// If needed in the future, we can implement this using aide's header support.
