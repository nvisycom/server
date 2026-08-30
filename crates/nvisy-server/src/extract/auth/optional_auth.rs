//! Optional authentication extractor for endpoints that vary by whether a caller
//! authenticated, without marking themselves auth-required in the OpenAPI spec.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, SecurityRequirement};
use axum::extract::{FromRef, FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use derive_more::{Deref, DerefMut};
use nvisy_postgres::PgClient;
use serde::Deserialize;

use super::AuthState;
use crate::handler::{Error, Result};
use crate::service::SessionKeys;

/// Optional [`AuthState`] for an endpoint that runs with or without a token.
///
/// Extracting a bare `Option<AuthState>` authenticates the same way, but its
/// generated OpenAPI security comes from the blanket `Option<T>` `OperationInput`,
/// which delegates to [`AuthState`] and so wrongly marks the operation
/// auth-required. This wrapper carries the same optional value while declaring the
/// token as *optional* in the spec (an empty requirement alongside the Bearer one,
/// so a public probe is not shown as needing a token). Use it for endpoints that
/// vary their behavior by whether a caller authenticated — e.g. the health check.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct OptionalAuth<T = ()>(pub Option<AuthState<T>>);

impl<T, S> FromRequestParts<S> for OptionalAuth<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
    S: Sync + Send + 'static,
    PgClient: FromRef<S>,
    SessionKeys: FromRef<S>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Reuses the optional extraction: a valid token authenticates, an absent or
        // invalid one yields `None` rather than rejecting.
        <AuthState<T> as OptionalFromRequestParts<S>>::from_request_parts(parts, state)
            .await
            .map(OptionalAuth)
    }
}

impl<T> OperationInput for OptionalAuth<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn operation_input(_ctx: &mut GenContext, operation: &mut Operation) {
        // Two alternatives: an empty requirement (no auth) and the Bearer one, so
        // the operation is documented as accessible with or without a token.
        operation.security = vec![
            SecurityRequirement::new(),
            [("BearerAuth".to_string(), vec![])].into(),
        ];
    }
}

#[cfg(test)]
mod tests {
    use aide::OperationInput;
    use aide::openapi::Operation;

    use super::OptionalAuth;

    /// The OpenAPI security for an optional-auth operation must offer an
    /// unauthenticated alternative (an empty requirement) alongside the Bearer
    /// one, so the endpoint is not documented as requiring a token — a public
    /// probe hitting the health check must not appear to need credentials.
    #[test]
    fn documents_auth_as_optional_not_required() {
        let mut operation = Operation::default();
        aide::generate::in_context(|ctx| {
            OptionalAuth::<()>::operation_input(ctx, &mut operation);
        });

        assert!(
            operation.security.iter().any(|req| req.is_empty()),
            "an empty requirement must be present so no auth also satisfies the operation",
        );
        assert!(
            operation
                .security
                .iter()
                .any(|req| req.contains_key("BearerAuth")),
            "the Bearer alternative must still be offered for authenticated callers",
        );
    }
}
