//! Redirects that return the browser to the configured frontend at the end of a
//! browser-driven flow (OIDC sign-in/link/reauth, cloud-file OAuth).
//!
//! Each flow validates its redirect target when it starts, so the `base` here is
//! always an allow-listed URL; when none is configured, a minimal self-describing
//! fallback is rendered instead of redirecting.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};

use crate::response::ErrorKind;

/// Tracing target for frontend-redirect construction.
const TRACING_TARGET: &str = "nvisy_server::response::redirect";

/// The outcome an OIDC callback conveys to the frontend, and where its value (if
/// any) is placed on the redirect URL.
pub(crate) enum RedirectResult<'a> {
    /// A plain success with no value (a completed link, or a web sign-in whose
    /// session rides in cookies set on the same response).
    Success,
    /// A failure.
    Error,
    /// A value carried in the URL **fragment** (`#{name}=…`) — for a *web* target,
    /// where a fragment is not sent to the server, not in `Referer`, and stays
    /// client-side. Used for the step-up reauth proof (a bearer credential the web
    /// frontend presents to a credential-adding action).
    Fragment { name: &'a str, value: &'a str },
    /// A value carried in the URL **query** (`?{name}=…`) — for a *desktop*
    /// custom-scheme deep-link, which has no server hop (so fragment vs query is
    /// moot for leakage) and where the query is the RFC 8252 native convention.
    /// Used for the desktop `app` token.
    Query { name: &'a str, value: &'a str },
}

impl RedirectResult<'_> {
    /// Returns the browser to `base` (the frontend) carrying this outcome.
    ///
    /// The `signin=success|error` status always goes in the query string. A
    /// carried value's placement depends on the target:
    /// [`Fragment`](RedirectResult::Fragment) for a web target (the reauth proof,
    /// kept out of the query so it does not leak via `Referer`/history),
    /// [`Query`](RedirectResult::Query) for a desktop custom-scheme deep-link (the
    /// `app` token, no server hop, query is the native convention). Web sign-in
    /// carries no value here: its session rides in cookies.
    ///
    /// `base` is only ever an allow-listed target (validated when the flow
    /// starts). When no target is configured, or it somehow fails to parse, this
    /// renders a minimal self-describing page instead of redirecting.
    pub(crate) fn into_redirect(self, base: Option<&str>) -> Response {
        enum Placement<'a> {
            None,
            Fragment(&'a str, &'a str),
            Query(&'a str, &'a str),
        }
        let (status, placement) = match self {
            RedirectResult::Success => ("success", Placement::None),
            RedirectResult::Error => ("error", Placement::None),
            RedirectResult::Fragment { name, value } => {
                ("success", Placement::Fragment(name, value))
            }
            RedirectResult::Query { name, value } => ("success", Placement::Query(name, value)),
        };
        let carries_value = !matches!(placement, Placement::None);

        // Build the redirect target through the URL parser so the query and
        // fragment are assembled and encoded correctly, rather than by string
        // concatenation that could mishandle an existing query or fragment.
        if let Some(base) = base
            && let Ok(mut url) = url::Url::parse(base)
        {
            url.query_pairs_mut().append_pair("signin", status);
            match placement {
                Placement::None => url.set_fragment(None),
                // A web bearer secret goes in the fragment, never the query, so it
                // is not leaked via Referer, history, or logs. `Url` encodes it.
                Placement::Fragment(name, value) => {
                    url.set_fragment(Some(&format!("{name}={value}")));
                }
                // A desktop deep-link value goes in the query (`query_pairs_mut`
                // percent-encodes it). The custom scheme has no server hop, so this
                // does not leak; it matches the native OAuth redirect convention.
                Placement::Query(name, value) => {
                    url.query_pairs_mut().append_pair(name, value);
                }
            }
            return Redirect::to(url.as_str()).into_response();
        }

        // No usable redirect target. A value-carrying outcome must NOT reach here:
        // its target was allow-listed at flow start, so a missing/unparseable base
        // now is a server-side invariant break — rendering the in-page page would
        // silently discard the token (leaving a desktop app hung) instead of
        // delivering it. Fail loudly rather than swallow it.
        if carries_value {
            tracing::error!(
                target: TRACING_TARGET,
                "callback reached the no-redirect fallback while carrying a token; \
                 the redirect target should have been validated at flow start",
            );
            return ErrorKind::InternalServerError
                .with_message("Sign-in could not be completed")
                .with_resource("authentication")
                .into_response();
        }

        // A valueless success/error with no configured frontend: render a minimal
        // in-page result.
        let body = format!("Sign-in {status}. You can close this window.");
        (StatusCode::OK, body).into_response()
    }
}

/// Returns the browser to the frontend after a cloud-file OAuth flow.
///
/// A `{workspaceSlug}` placeholder in the configured base is substituted with
/// `workspace_slug` when known (i.e. on success), so a base like
/// `https://app/w/{workspaceSlug}/integrations` lands on the workspace's page.
/// The outcome is appended as a `connection=success|error` query. When no
/// frontend URL is configured, renders a minimal in-page result instead of a
/// redirect — a browser blocks a top-level navigation to a `data:` URL, so a
/// `data:` `Location` would show the user nothing.
pub(crate) fn connection_result_redirect(
    base: Option<&str>,
    status: &str,
    workspace_slug: Option<&str>,
) -> Response {
    match base {
        Some(base) => {
            let base = match workspace_slug {
                Some(slug) => base.replace("{workspaceSlug}", slug),
                None => base.to_owned(),
            };
            let separator = if base.contains('?') { '&' } else { '?' };
            Redirect::to(&format!("{base}{separator}connection={status}")).into_response()
        }
        None => {
            let body = format!("Cloud file connection {status}. You can close this window.");
            (StatusCode::OK, body).into_response()
        }
    }
}
