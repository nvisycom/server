//! CSRF-protection middleware for cookie-authenticated requests.

use axum::extract::Request;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::CookieJar;

use super::TRACING_TARGET;
use crate::extract::{AuthHeader, AuthTransport, CSRF_COOKIE_NAME, CSRF_HEADER_NAME};
use crate::handler::{ErrorKind, Result};

/// Enforces CSRF protection on cookie-authenticated, state-changing requests
/// (the double-submit-cookie check).
///
/// The check applies only when **both** hold:
/// - the request authenticated via the **session cookie** (a `Bearer`-header
///   request is not CSRF-exposed — the browser never attaches that header on its
///   own — so it is exempt), and
/// - the method is **state-changing** (`POST`/`PUT`/`PATCH`/`DELETE`); safe
///   methods (`GET`/`HEAD`/`OPTIONS`) are exempt.
///
/// When it applies, the request must carry the CSRF token both as the
/// [`CSRF_HEADER_NAME`] header and the [`CSRF_COOKIE_NAME`] cookie, and the two
/// must match. A cross-site attacker can cause the browser to send the cookie but
/// cannot read it to set the matching header (it is same-origin readable only),
/// nor set a custom header cross-site, so a forged request fails the match.
///
/// Runs after authentication, so the transport that authenticated is known (the
/// [`AuthHeader`] the auth layer verified and cached on the request).
pub async fn csrf_protect(request: Request, next: Next) -> Result<Response> {
    // Safe methods never mutate state, so they are exempt regardless of transport.
    let is_state_changing = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    // The transport is read from the verified `AuthHeader` the auth layer cached.
    // Its absence means this route was not authenticated (no auth layer ran), so
    // there is no cookie session to protect — CSRF does not apply.
    let via_cookie = request
        .extensions()
        .get::<AuthHeader>()
        .and_then(AuthHeader::transport)
        == Some(AuthTransport::Cookie);

    if is_state_changing && via_cookie {
        let jar = CookieJar::from_headers(request.headers());
        let cookie_token = jar.get(CSRF_COOKIE_NAME).map(|c| c.value());
        let header_token = request
            .headers()
            .get(CSRF_HEADER_NAME)
            .and_then(|value| value.to_str().ok());

        if !csrf_tokens_match(cookie_token, header_token) {
            tracing::warn!(
                target: TRACING_TARGET,
                method = %request.method(),
                "CSRF check failed on cookie-authenticated request"
            );
            return Err(ErrorKind::Forbidden
                .with_message("CSRF check failed")
                .with_context("Missing or mismatched CSRF token")
                .with_resource("authentication"));
        }
    }

    Ok(next.run(request).await)
}

/// The double-submit CSRF match: the header and cookie tokens must both be
/// present, non-empty, and equal. A cross-site attacker can make the browser
/// attach the cookie but cannot read it to set the matching header, so a forged
/// request fails this check.
///
/// The comparison is constant-time in the token bytes (it inspects every byte
/// rather than short-circuiting on the first mismatch), so it does not leak
/// how much of a guessed token matched via response timing.
fn csrf_tokens_match(cookie_token: Option<&str>, header_token: Option<&str>) -> bool {
    match (cookie_token, header_token) {
        (Some(cookie), Some(header)) => !cookie.is_empty() && constant_time_eq(cookie, header),
        _ => false,
    }
}

/// Constant-time byte-equality: the running time depends only on the input
/// lengths, not on where (or whether) the bytes first differ. Unequal lengths
/// return `false` immediately — length is not itself secret here.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::{constant_time_eq, csrf_tokens_match};

    #[test]
    fn constant_time_eq_matches_string_equality() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        // Different lengths are unequal (never a prefix match).
        assert!(!constant_time_eq("abc", "abc123"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn csrf_matches_only_when_both_present_nonempty_and_equal() {
        // Matching non-empty tokens pass.
        assert!(csrf_tokens_match(Some("abc123"), Some("abc123")));

        // Mismatched tokens fail.
        assert!(!csrf_tokens_match(Some("abc123"), Some("def456")));

        // A missing side (header not echoed, or no cookie) fails.
        assert!(!csrf_tokens_match(Some("abc123"), None));
        assert!(!csrf_tokens_match(None, Some("abc123")));
        assert!(!csrf_tokens_match(None, None));

        // Empty tokens never match, even if equal — an attacker could set an empty
        // header to pair a stripped cookie.
        assert!(!csrf_tokens_match(Some(""), Some("")));
    }
}
