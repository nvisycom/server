use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::extract::AuthState;

/// Requires a valid authentication token to proceed with the request.
///
/// #### Notes
///
/// - [`AuthHeader`](crate::extract::AuthHeader) can't be extracted from requests without `Authorization` header.
/// - [`AuthState`] can't be extracted from requests without *verified* `Authorization` token.
///
/// #### Examples
///
/// ```rust,no_run
/// use axum::middleware::from_fn_with_state;
/// use nvisy_server::middleware::require_authentication;
/// use nvisy_server::service::{ServiceConfig, ServiceState};
///
/// let state = ServiceState::from_config(&ServiceConfig::default()).await?;
/// let _guard = from_fn_with_state(state, require_authentication);
/// ```
pub async fn require_authentication(
    AuthState(_): AuthState,
    request: Request,
    next: Next,
) -> Response {
    next.run(request).await
}
