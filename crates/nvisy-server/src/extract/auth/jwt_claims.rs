//! JWT claims structure and token encoding/decoding.
//!
//! This module provides [`AuthClaims`], the core JWT claims structure used for
//! authentication tokens. It handles token creation, validation, encoding, and
//! decoding with comprehensive security checks.

use std::borrow::Cow;

use jiff::{Span, Timestamp};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use nvisy_postgres::model::{Account, AccountApiToken};
use nvisy_postgres::types::{ApiTokenType, session};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::response::{ErrorKind, Result};

/// Tracing target for authentication operations.
const TRACING_TARGET: &str = "nvisy_server::authentication";

/// Far-future timestamp for tokens that never expire (100 years from now).
const NEVER_EXPIRES_SECONDS: i64 = 100 * 365 * 24 * 60 * 60;

/// JWT claims for authentication tokens.
///
/// This structure contains both RFC 7519 standard JWT claims and service-specific claims.
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

    /// Issued at (as Unix timestamp).
    #[serde(rename = "iat")]
    pub issued_at: i64,
    /// Expiration time (as Unix timestamp).
    #[serde(rename = "exp")]
    pub expires_at: i64,

    // Private (or custom) claims
    #[serde(flatten)]
    pub custom_claims: T,
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
        let issued_at = Timestamp::from(account_api_token.issued_at);

        // The JWT's `exp` depends on the token kind:
        //
        // - `web` browser sessions: the session's ABSOLUTE cap (`issued_at +
        //   MAX_AGE`), NOT the row's sliding idle bound (`expired_at`). This lets
        //   the idle bound slide freely on the row without the JWT lapsing
        //   mid-session, while the JWT still independently guarantees no browser
        //   session outlives the absolute cap even if the row check were bypassed.
        // - `api` programmatic tokens and `app` desktop sessions: the token's own
        //   `expired_at` (the chosen lifetime), or a far-future value when it never
        //   expires. The browser absolute cap does not apply to them.
        //
        // Either way the row remains the authority for idle expiry and revocation;
        // the JWT `exp` is a backstop.
        let never_expires = || Timestamp::now() + Span::new().seconds(NEVER_EXPIRES_SECONDS);
        let expires_at = match account_api_token.session_type {
            ApiTokenType::Web => issued_at
                .checked_add(Span::new().seconds(session::MAX_AGE.as_secs() as i64))
                .unwrap_or_else(|_| never_expires()),
            ApiTokenType::Api | ApiTokenType::App => account_api_token
                .expired_at
                .map_or_else(never_expires, Timestamp::from),
        };

        Self {
            issued_by: Cow::Borrowed(Self::JWT_ISSUER),
            audience: Cow::Borrowed(Self::JWT_AUDIENCE),
            token_id: account_api_token.id,
            account_id: account_model.id,
            issued_at: issued_at.as_second(),
            expires_at: expires_at.as_second(),
            custom_claims,
        }
    }
}

impl<T> AuthClaims<T>
where
    T: Clone + Serialize,
{
    /// Encodes the claims into a signed JWT token string.
    ///
    /// # Arguments
    ///
    /// * `encoding_key` - The private key for token signing
    ///
    /// # Returns
    ///
    /// Returns the encoded JWT token string.
    ///
    /// # Errors
    ///
    /// Returns errors for JWT encoding failures.
    pub fn into_string(self, encoding_key: &EncodingKey) -> Result<String> {
        let header = Header::new(Algorithm::EdDSA);
        encode(&header, &self, encoding_key).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %e,
                account_id = %self.account_id,
                "Failed to encode JWT token"
            );

            ErrorKind::InternalServerError
                .with_message("Authentication token generation failed")
                .with_context("Unable to create session token")
                .with_resource("authentication")
        })
    }
}

impl<T> AuthClaims<T>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    /// Parses and validates a raw JWT token string, regardless of the transport
    /// it arrived on (session cookie or Authorization header).
    ///
    /// This method performs comprehensive validation including:
    /// - Signature verification using EdDSA
    /// - Standard JWT claims validation (iss, aud, exp, etc.)
    /// - Application-specific claim presence
    /// - Expiration checking with detailed logging
    ///
    /// # Arguments
    ///
    /// * `auth_token` - The raw JWT string
    /// * `decoding_key` - The public key for signature verification
    ///
    /// # Returns
    ///
    /// Returns validated [`AuthClaims`] on success.
    ///
    /// # Errors
    ///
    /// Returns various authentication errors for invalid tokens.
    pub fn from_token(auth_token: &str, decoding_key: &DecodingKey) -> Result<Self> {
        // Configure comprehensive JWT validation
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = true;
        // No clock-skew grace on `exp`: the JWT expiry is documented as an
        // independent absolute cap (a backstop if the DB session check is ever
        // bypassed), and the default 60s leeway would let a token past its cap
        // through for up to a minute.
        validation.leeway = 0;
        validation.validate_nbf = false; // Not Before claim not used
        validation.validate_aud = true;
        validation.set_audience(&[Self::JWT_AUDIENCE]);
        validation.set_issuer(&[Self::JWT_ISSUER]);
        validation.set_required_spec_claims(&["iss", "aud", "jti", "sub", "iat", "exp"]);

        tracing::debug!(
            target: TRACING_TARGET,
            audience = Self::JWT_AUDIENCE,
            issuer = Self::JWT_ISSUER,
            "Validating JWT token with strict security settings"
        );

        let token_data = decode::<Self>(auth_token, decoding_key, &validation).map_err(|e| {
            tracing::warn!(
                target: TRACING_TARGET,
                error = %e,
                error_kind = ?e.kind(),
                "JWT token decode failed"
            );
            e
        })?;
        let claims = token_data.claims;

        // `validate_exp` above makes `decode` reject an expired token, so reaching
        // here means the token is within its lifetime — no manual re-check needed.
        tracing::debug!(
            target: TRACING_TARGET,
            token_id = %claims.token_id,
            account_id = %claims.account_id,
            "JWT token validation completed successfully"
        );

        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use nvisy_postgres::model::{Account, AccountApiToken};
    use nvisy_postgres::types::{ApiTokenType, session};

    use super::{AuthClaims, NEVER_EXPIRES_SECONDS};

    #[test]
    fn web_exp_is_the_absolute_cap_from_issued_at() {
        let account = Account::test();
        let token = AccountApiToken::test(account.id, ApiTokenType::Web);
        let claims = AuthClaims::new(&account, &token);

        // A web session's JWT `exp` is `issued_at + MAX_AGE`, independent of the
        // row's (sliding) `expired_at`.
        let issued = Timestamp::from(token.issued_at).as_second();
        let expected = issued + session::MAX_AGE.as_secs() as i64;
        assert_eq!(claims.expires_at, expected);
        assert_eq!(claims.account_id, account.id);
        assert_eq!(claims.token_id, token.id);
    }

    #[test]
    fn api_and_app_exp_follow_the_rows_own_expiry() {
        let account = Account::test();
        let chosen = Timestamp::now() + Span::new().hours(3);

        for kind in [ApiTokenType::Api, ApiTokenType::App] {
            let mut token = AccountApiToken::test(account.id, kind);
            token.expired_at = Some(chosen.into());
            let claims = AuthClaims::new(&account, &token);
            // Not the browser cap — the token's chosen lifetime.
            assert_eq!(claims.expires_at, chosen.as_second());
        }
    }

    #[test]
    fn api_and_app_without_expiry_never_effectively_expire() {
        let account = Account::test();
        let token = AccountApiToken::test(account.id, ApiTokenType::Api); // expired_at: None
        let claims = AuthClaims::new(&account, &token);

        // Falls back to a far-future value (~100 years), so the JWT never lapses.
        let lower_bound = Timestamp::now().as_second() + NEVER_EXPIRES_SECONDS - 60;
        assert!(claims.expires_at >= lower_bound);
    }
}
