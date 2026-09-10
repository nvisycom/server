//! Session-token extraction.
//!
//! Provides [`SessionToken`], the extractor that reads and validates the session
//! JWT from an incoming request — a session cookie (browser) or an
//! `Authorization: Bearer` header (programmatic) — and records which transport
//! carried it. Signing outbound tokens is [`AuthIssuer`](crate::service::AuthIssuer).

use std::fmt::Debug;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum_extra::TypedHeader;
use axum_extra::extract::CookieJar;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use axum_extra::typed_header::TypedHeaderRejectionReason;
use jsonwebtoken::errors::{Error as JwtError, ErrorKind as JwtErrorKind};
use serde::Deserialize;

use super::AuthClaims;
use crate::extract::auth::SESSION_COOKIE_NAME;
use crate::response::{Error, ErrorKind, Result};
use crate::service::SessionKeys;

/// Which transport carried the session token on a request.
///
/// The same signed JWT reaches the server either in an `HttpOnly` session cookie
/// (the browser SPA) or in an `Authorization: Bearer` header (programmatic API /
/// SDK callers). The distinction is retained because CSRF protection applies only
/// to cookie-authenticated requests — a bearer request is not CSRF-exposed, since
/// the browser never attaches an `Authorization` header on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthTransport {
    /// The token came from the session cookie (browser).
    Cookie,
    /// The token came from the `Authorization: Bearer` header (API / SDK).
    Bearer,
}

/// The verified session credential together with the transport that carried it.
///
/// Read from a request, it validates the session JWT (from the session cookie or
/// an `Authorization: Bearer` header) and records which transport delivered it —
/// the distinction CSRF protection depends on. Signing outbound tokens is not its
/// job; that is [`AuthIssuer`](crate::service::AuthIssuer).
///
/// # Security
///
/// The JWT is validated for signature integrity, expiration, the required claims
/// (iss, aud, jti, sub, iat, exp), and issuer/audience matching.
///
/// # Notes
///
/// This extractor only performs JWT validation. For full authentication including
/// database verification, use [`AuthState`] instead.
///
/// [`AuthState`]: crate::extract::AuthState
#[must_use]
#[derive(Debug, Clone)]
pub struct SessionToken<T = ()> {
    auth_claims: AuthClaims<T>,
    /// The transport the token arrived on.
    transport: AuthTransport,
}

impl<T> SessionToken<T> {
    /// The transport this token arrived on.
    #[inline]
    #[must_use]
    pub const fn transport(&self) -> AuthTransport {
        self.transport
    }

    /// Consumes this token and returns the verified JWT claims.
    #[inline]
    pub fn into_auth_claims(self) -> AuthClaims<T> {
        self.auth_claims
    }
}

impl<T> SessionToken<T>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    /// Validates a raw JWT `token` carried by `transport` (signature, claims,
    /// expiry) and records the transport.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid, expired, or malformed.
    fn from_token(token: &str, transport: AuthTransport, keys: &SessionKeys) -> Result<Self> {
        let auth_claims = AuthClaims::from_token(token, keys.decoding_key())?;
        Ok(Self {
            auth_claims,
            transport,
        })
    }
}

impl<T, S> FromRequestParts<S> for SessionToken<T>
where
    T: Clone + for<'de> Deserialize<'de> + Send + Sync + 'static,
    S: Sync + Send,
    SessionKeys: FromRef<S>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Return the cached token if a prior extractor already verified it.
        if let Some(session_token) = parts.extensions.get::<Self>() {
            return Ok(session_token.clone());
        }

        let auth_keys = SessionKeys::from_ref(state);

        // The session token can arrive two ways. Prefer the session cookie (the
        // browser SPA), then fall back to the `Authorization: Bearer` header
        // (programmatic API / SDK callers). The verified claims are identical
        // either way; only the recorded transport differs, which downstream CSRF
        // protection depends on.
        let jar = CookieJar::from_headers(&parts.headers);
        let cookie_token = jar
            .get(SESSION_COOKIE_NAME)
            .map(|cookie| cookie.value().to_owned());

        let session_token = if let Some(token) = cookie_token {
            Self::from_token(&token, AuthTransport::Cookie, &auth_keys)?
        } else {
            // No session cookie: require a Bearer header.
            type AuthBearerHeader = TypedHeader<Authorization<Bearer>>;
            let bearer = AuthBearerHeader::from_request_parts(parts, state)
                .await
                .map_err(|rejection| match rejection.reason() {
                    TypedHeaderRejectionReason::Missing => ErrorKind::MissingAuthToken
                        .with_message("Authentication required")
                        .with_context("Provide a session cookie or a Bearer token")
                        .with_resource("authentication"),
                    TypedHeaderRejectionReason::Error(_) => ErrorKind::MalformedAuthToken
                        .with_message("Invalid token format")
                        .with_context("Authorization header must contain a valid Bearer token")
                        .with_resource("authentication"),
                    _ => ErrorKind::InternalServerError
                        .with_message("Authentication processing failed")
                        .with_context("Unexpected error during header extraction")
                        .with_resource("authentication"),
                })?;
            Self::from_token(bearer.token(), AuthTransport::Bearer, &auth_keys)?
        };

        // Cache for subsequent extractors in the same request.
        parts.extensions.insert(session_token.clone());
        Ok(session_token)
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
