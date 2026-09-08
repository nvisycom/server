//! Building the session and CSRF cookies for browser (cookie-transport) clients.
//!
//! A browser signs in and receives the session JWT in an `HttpOnly` cookie
//! ([`SESSION_COOKIE_NAME`]) rather than in the response body, so the token is
//! never exposed to page script. Alongside it, a readable CSRF token cookie
//! ([`CSRF_COOKIE_NAME`]) is set for the double-submit check enforced on
//! state-changing requests.
//!
//! Programmatic (API / SDK) callers do not use cookies — they receive the JWT in
//! the response body and send it as an `Authorization: Bearer` header — so these
//! helpers are used only on the cookie-transport paths (browser login/signup and
//! the OIDC callback).
//!
//! [`SESSION_COOKIE_NAME`]: crate::extract::SESSION_COOKIE_NAME
//! [`CSRF_COOKIE_NAME`]: crate::extract::CSRF_COOKIE_NAME

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

impl CookieConfig {
    /// The maximum age applied to session/CSRF cookies: the session's absolute
    /// cap. The idle bound is enforced server-side against the session row, so the
    /// cookie itself only needs to survive up to the hard age limit.
    fn max_age() -> time::Duration {
        time::Duration::seconds(session::MAX_AGE.as_secs() as i64)
    }

    /// Builds the `HttpOnly` session cookie carrying `jwt`.
    ///
    /// `HttpOnly` keeps page script from reading the token; `Secure` (per config)
    /// restricts it to HTTPS; `SameSite=Lax` lets it ride the provider's top-level
    /// redirect back to the app (needed for the OIDC callback) while still not
    /// being sent on cross-site background requests.
    fn session_cookie(self, jwt: String) -> Cookie<'static> {
        Cookie::build((SESSION_COOKIE_NAME, jwt))
            .http_only(true)
            .secure(self.secure)
            .same_site(SameSite::Lax)
            .path("/")
            .max_age(Self::max_age())
            .build()
    }

    /// Builds the readable CSRF-token cookie for the double-submit check.
    ///
    /// Deliberately **not** `HttpOnly`: the SPA reads it and echoes it in the CSRF
    /// header on state-changing requests. It is not a secret credential on its
    /// own — it is only meaningful paired with the `HttpOnly` session cookie an
    /// attacker cannot read or set cross-site.
    fn csrf_cookie(self, token: String) -> Cookie<'static> {
        Cookie::build((CSRF_COOKIE_NAME, token))
            .http_only(false)
            .secure(self.secure)
            .same_site(SameSite::Lax)
            .path("/")
            .max_age(Self::max_age())
            .build()
    }

    /// A `CookieJar` holding a freshly minted session's cookies: the `HttpOnly`
    /// session cookie carrying `jwt`, plus a paired CSRF cookie with a new token.
    ///
    /// Used both by [`session_response`](Self::session_response) (a plain sign-in)
    /// and by the OIDC callback, which attaches the jar to a redirect.
    pub fn session_jar(self, jwt: String) -> CookieJar {
        CookieJar::new()
            .add(self.session_cookie(jwt))
            .add(self.csrf_cookie(generate_csrf_token()))
    }

    /// A `204 No Content` sign-in response that sets the session and CSRF cookies
    /// for `jwt`. The token is delivered only in the `HttpOnly` cookie, never in
    /// the body, so browser page script cannot read it.
    #[must_use]
    pub fn session_response(self, jwt: String) -> Response {
        (StatusCode::NO_CONTENT, self.session_jar(jwt)).into_response()
    }

    /// The pair of cookies that clear a browser session on logout. Empty value and
    /// immediate expiry; attributes match the originals so the browser overwrites
    /// them.
    pub fn clearing_response_jar(self) -> CookieJar {
        CookieJar::new()
            .add(self.clearing_cookie(SESSION_COOKIE_NAME, true))
            .add(self.clearing_cookie(CSRF_COOKIE_NAME, false))
    }

    fn clearing_cookie(self, name: &'static str, http_only: bool) -> Cookie<'static> {
        Cookie::build((name, ""))
            .http_only(http_only)
            .secure(self.secure)
            .same_site(SameSite::Lax)
            .path("/")
            .max_age(time::Duration::ZERO)
            .build()
    }
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
