//! OIDC sign-in error to HTTP error conversion.
//!
//! Maps [`OidcError`] onto HTTP errors so a sign-in failure surfaces with an
//! appropriate status. A verification failure is a client-facing 401 (the
//! provider's assertion did not check out), a misconfigured or absent provider
//! is a server-side condition, and discovery/exchange failures against the
//! provider are upstream faults surfaced as 502.

use super::http_error::{Error as HttpError, ErrorKind};
use crate::service::OidcError;

impl<'a> From<OidcError> for HttpError<'a> {
    fn from(error: OidcError) -> Self {
        let message = error.to_string();
        match error {
            // The provider is not enabled on this deployment: the caller asked for
            // a sign-in method that does not exist here.
            OidcError::ProviderNotConfigured(_) => ErrorKind::NotFound
                .with_message("Sign-in provider is not available")
                .with_context(message),
            // Static misconfiguration should have failed at startup; if it
            // surfaces here it is a server fault, not the caller's.
            OidcError::Config { .. } => ErrorKind::InternalServerError
                .with_message("Sign-in provider is misconfigured")
                .with_context(message),
            // Discovery and code exchange are calls to the provider; a failure is
            // an upstream fault rather than a bad request from our caller, so it
            // is surfaced as a retryable service-unavailable.
            OidcError::Discovery { .. } | OidcError::Exchange { .. } => {
                ErrorKind::ServiceUnavailable
                    .with_message("Sign-in provider did not complete the exchange")
                    .with_context(message)
            }
            // A missing or unverifiable ID token means the sign-in cannot be
            // trusted: reject it as unauthorized.
            OidcError::MissingIdToken(_) | OidcError::Verification { .. } => {
                ErrorKind::Unauthorized
                    .with_message("Sign-in could not be verified")
                    .with_context(message)
            }
        }
    }
}
