use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::extract::AuthState;
use crate::handler::ErrorKind;

/// Requires the authenticated account to have administrator privileges.
///
/// #### Notes
///
/// - [`AuthState`] can't be extracted from requests without *verified* `Authorization` token.
/// - See [`require_authentication`](super::require_authentication) for more information.
pub async fn require_admin(
    AuthState(auth_claims): AuthState,
    request: Request,
    next: Next,
) -> Response {
    if !auth_claims.is_administrator {
        return ErrorKind::Unauthorized.into_response();
    }

    next.run(request).await
}
