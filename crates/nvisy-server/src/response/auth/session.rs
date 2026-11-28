//! Browser-session cookie emission for cookie-transport (browser) clients.
//!
//! A browser signs in and receives the session JWT in an `HttpOnly` cookie
//! ([`SESSION_COOKIE_NAME`]) rather than in the response body, so the token is
//! never exposed to page script. Alongside it, a readable CSRF token cookie
//! ([`CSRF_COOKIE_NAME`]) is set for the double-submit check enforced on
//! state-changing requests.
//!
//! [`CookieConfig`] is the deployment policy (just the `Secure` attribute).
//! [`WebSession`] emits the cookies for a freshly minted session, and
//! [`ClearedSession`] emits the pair that clears them on sign-out. Programmatic
//! (API / SDK) callers do not use cookies — they receive the JWT in the response
//! body and send it as an `Authorization: Bearer` header — so these types are
//! used only on the cookie-transport paths (browser login/signup, the OIDC
//! callback, and logout).
//!
//! [`SESSION_COOKIE_NAME`]: crate::extract::SESSION_COOKIE_NAME
//! [`CSRF_COOKIE_NAME`]: crate::extract::CSRF_COOKIE_NAME

use aide::OperationOutput;
use aide::generate::GenContext;
use aide::openapi::Operation;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use base64::Engine;
use nvisy_postgres::types::session;

use crate::extract::{CSRF_COOKIE_NAME, SESSION_COOKIE_NAME};

/// Session-cookie policy: the deployment-dependent attributes applied to the
/// session and CSRF cookies.
///
/// The only knob is [`secure`](Self::secure). It is `true` by default and in
/// production: a `Secure` cookie is only sent over HTTPS, which is required for a
/// bearer credential. It must be set to `false` for local development served over
/// plain HTTP, where browsers silently drop `Secure` cookies and the session would
/// never be set.
///
/// This is pure policy; the cookies themselves are built by [`WebSession`] and
/// [`ClearedSession`].
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct CookieConfig {
    /// Whether session cookies carry the `Secure` attribute (HTTPS-only).
    ///
    /// Keep `true` in production. Set `false` only for local HTTP development, or
    /// the browser will not store the cookie.
    #[cfg_attr(
        feature = "cli",
        arg(long = "cookie-secure", env = "COOKIE_SECURE", default_value_t = true)
    )]
    pub secure: bool,
}

impl Default for CookieConfig {
    fn default() -> Self {
        // Production-safe default: `Secure` on. Dev over HTTP opts out explicitly.
        Self { secure: true }
    }
}

/// The maximum age applied to session/CSRF cookies: the session's absolute cap.
/// The idle bound is enforced server-side against the session row, so the cookie
/// itself only needs to survive up to the hard age limit.
fn cookie_max_age() -> time::Duration {
    time::Duration::seconds(session::MAX_AGE_SECS)
}

/// Builds a session-scoped cookie: `SameSite=Lax`, the whole site, `Secure` per
/// deployment. The session cookie is `HttpOnly`; the CSRF cookie deliberately is
/// not — the SPA reads it and echoes it in the CSRF header, and it is only
/// meaningful paired with the `HttpOnly` session cookie an attacker cannot read
/// or set cross-site. A zero `max_age` clears the cookie.
fn cookie(
    name: &'static str,
    value: String,
    http_only: bool,
    secure: bool,
    max_age: time::Duration,
) -> Cookie<'static> {
    Cookie::build((name, value))
        .http_only(http_only)
        .secure(secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(max_age)
        .build()
}

/// Generates an unguessable CSRF token: URL-safe base64 of 32 CSPRNG bytes.
///
/// The value is a bearer-grade random string, so its bytes come from a
/// cryptographically secure RNG (`rand::rng()`), never a fast non-crypto one.
fn generate_csrf_token() -> String {
    use rand::Rng;

    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// A freshly minted browser session, ready to be delivered as `Set-Cookie`.
///
/// Carries the session JWT and the deployment [`CookieConfig`]. It renders either
/// as a bare `204` sign-in response (via [`IntoResponse`]) or as a jar of cookies
/// to attach to another response (the OIDC callback attaches it to a redirect).
/// The token is delivered only in the `HttpOnly` cookie, never in a body.
#[must_use]
pub struct WebSession {
    jwt: String,
    config: CookieConfig,
}

impl WebSession {
    /// Wraps a minted session JWT with the cookie policy that will deliver it.
    #[inline]
    pub const fn new(jwt: String, config: CookieConfig) -> Self {
        Self { jwt, config }
    }

    /// The session and CSRF cookies as a jar, to attach to a caller-built
    /// response — used by the OIDC callback, which sets them on a redirect.
    pub fn into_jar(self) -> CookieJar {
        CookieJar::new()
            .add(cookie(
                SESSION_COOKIE_NAME,
                self.jwt,
                true,
                self.config.secure,
                cookie_max_age(),
            ))
            .add(cookie(
                CSRF_COOKIE_NAME,
                generate_csrf_token(),
                false,
                self.config.secure,
                cookie_max_age(),
            ))
    }
}

impl IntoResponse for WebSession {
    /// A `204 No Content` sign-in response that sets the session and CSRF cookies.
    fn into_response(self) -> Response {
        (StatusCode::NO_CONTENT, self.into_jar()).into_response()
    }
}

impl OperationOutput for WebSession {
    type Inner = ();

    fn operation_response(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Option<aide::openapi::Response> {
        // The sign-in response carries no body (204 + Set-Cookie); the concrete
        // responses are documented on each operation via TransformOperation.
        None
    }

    fn inferred_responses(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, aide::openapi::Response)> {
        // Prevent aide from inferring a default 200; the 204 is documented explicitly.
        Vec::new()
    }
}

/// The cookies that clear a browser session on sign-out: the session and CSRF
/// cookies emptied and immediately expired.
///
/// Logout performs token revocation and returns its own status, so this exposes
/// only [`into_jar`](Self::into_jar) (attached to that response) rather than an
/// [`IntoResponse`] of its own.
#[must_use]
pub struct ClearedSession {
    config: CookieConfig,
}

impl ClearedSession {
    /// A clearing effect under the given cookie policy.
    #[inline]
    pub const fn new(config: CookieConfig) -> Self {
        Self { config }
    }

    /// The clearing session and CSRF cookies as a jar.
    pub fn into_jar(self) -> CookieJar {
        CookieJar::new()
            .add(cookie(
                SESSION_COOKIE_NAME,
                String::new(),
                true,
                self.config.secure,
                time::Duration::ZERO,
            ))
            .add(cookie(
                CSRF_COOKIE_NAME,
                String::new(),
                false,
                self.config.secure,
                time::Duration::ZERO,
            ))
    }
}
