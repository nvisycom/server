//! Session middleware.
//!
//! [`require_authentication`] gates a route on a valid session (the [`AuthState`]
//! extractor does the credential extraction and database verification).
//! [`slide_session`] extends the session's idle bound on use. They are separate
//! layers so CSRF protection can run *between* them — after authentication has
//! resolved the transport, but before the session-extending write.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::AccountApiTokenRepository;
use nvisy_postgres::types::session::SlidingWindow;

use super::TRACING_TARGET;
use crate::extract::AuthState;
use crate::handler::Result;

/// Requires a valid session to proceed with the request.
///
/// The [`AuthState`] extractor performs credential extraction (session cookie or
/// Bearer token) and full database verification; reaching the body means the
/// request is authenticated.
pub async fn require_authentication(_: AuthState, request: Request, next: Next) -> Response {
    next.run(request).await
}

/// Slides a browser session's idle bound forward on use.
///
/// Session validity (existence, revocation, idle and absolute expiry) is decided
/// authoritatively by [`AuthState`]'s database check when the extractor runs, so
/// reaching this body means the session is already valid — this middleware only
/// extends it. It must be layered *inside* CSRF protection, so a request that will
/// be rejected for a missing CSRF token never reaches the session-extending write.
pub async fn slide_session(
    auth_state: AuthState,
    State(pg_database): State<PgClient>,
    request: Request,
    next: Next,
) -> Result<Response> {
    // Slide the session's idle bound forward on use, throttled so it writes at
    // most once per throttle interval rather than on every request. Only `web`
    // sessions actually slide (the query no-ops for programmatic tokens). This is
    // best-effort keep-alive only: a failure to slide must not fail the request —
    // it just means this request did not extend the session — so a database error
    // here is logged and swallowed.
    //
    // Scope the connection to just this write and release it before running the
    // downstream request: `next.run` drives the whole handler (extractors, slow
    // upload streaming, and their own connection use), so holding this pooled
    // connection across it would pin one connection per in-flight request for the
    // request's entire lifetime and exhaust the pool under concurrent load.
    {
        match pg_database.get_connection().await {
            Ok(mut conn) => {
                if let Err(error) = conn
                    .slide_account_api_token(auth_state.token_id, SlidingWindow::standard())
                    .await
                {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        error = %error,
                        account_id = %auth_state.account_id,
                        token_id = %auth_state.token_id,
                        "failed to slide session; session left unextended this request"
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %error,
                    account_id = %auth_state.account_id,
                    token_id = %auth_state.token_id,
                    "could not acquire a connection to slide session; left unextended"
                );
            }
        }
    }

    Ok(next.run(request).await)
}
