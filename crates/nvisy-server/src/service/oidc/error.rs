//! Errors raised by the OIDC sign-in service.

use nvisy_postgres::types::IdentityProvider;

/// A failure during an OIDC sign-in.
#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    /// The requested provider is not configured on this deployment.
    #[error("OIDC provider is not configured: {0}")]
    ProviderNotConfigured(IdentityProvider),

    /// A provider's static configuration is invalid (issuer or redirect URI).
    #[error("invalid OIDC configuration: {message}")]
    Config {
        /// What was invalid.
        message: String,
        /// The underlying parse error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Provider metadata (or JWKS) discovery failed.
    #[error("OIDC discovery failed for {provider}: {message}")]
    Discovery {
        /// The provider being discovered.
        provider: IdentityProvider,
        /// The underlying failure.
        message: String,
    },

    /// Exchanging the authorization code for tokens failed.
    #[error("OIDC code exchange failed for {provider}: {message}")]
    Exchange {
        /// The provider the exchange was against.
        provider: IdentityProvider,
        /// The underlying failure.
        message: String,
    },

    /// The token response carried no ID token, so the sign-in cannot be verified.
    #[error("OIDC provider {0} returned no ID token")]
    MissingIdToken(IdentityProvider),

    /// ID-token verification failed (signature, `aud`, `iss`, `exp`, or `nonce`).
    #[error("OIDC ID token verification failed for {provider}: {message}")]
    Verification {
        /// The provider whose token failed verification.
        provider: IdentityProvider,
        /// The underlying failure.
        message: String,
    },
}

impl OidcError {
    /// Builds a [`Config`](Self::Config) error from a message and a source error.
    pub(super) fn config<E>(message: &str, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Config {
            message: message.to_owned(),
            source: Box::new(source),
        }
    }

    /// Builds a [`Discovery`](Self::Discovery) error.
    pub(super) fn discovery<E: std::fmt::Display>(provider: IdentityProvider, source: E) -> Self {
        Self::Discovery {
            provider,
            message: source.to_string(),
        }
    }

    /// Builds an [`Exchange`](Self::Exchange) error.
    pub(super) fn exchange(provider: IdentityProvider, message: String) -> Self {
        Self::Exchange { provider, message }
    }

    /// Builds a [`Verification`](Self::Verification) error.
    pub(super) fn verification(provider: IdentityProvider, message: String) -> Self {
        Self::Verification { provider, message }
    }
}
