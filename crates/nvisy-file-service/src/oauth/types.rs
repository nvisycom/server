//! The data types the OAuth flow is described and driven by.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The OAuth2 endpoints and scopes for one provider (e.g. Google).
#[derive(Debug, Clone)]
pub struct OAuthProvider {
    /// Authorization endpoint (where the user is sent to grant access).
    pub auth_url: String,
    /// Token endpoint (where codes and refresh tokens are exchanged).
    pub token_url: String,
    /// Scopes to request. Some providers (e.g. Box) configure scopes on the app
    /// rather than in the authorize URL, and leave this empty.
    pub scopes: Vec<String>,
    /// Extra provider-specific authorize-URL parameters needed to obtain a
    /// refresh token (e.g. Google's `access_type=offline`, Dropbox's
    /// `token_access_type=offline`).
    pub extra_authorize_params: Vec<(String, String)>,
}

/// The OAuth application's credentials and redirect, supplied by configuration.
#[derive(Debug, Clone)]
pub struct OAuthApp {
    /// The OAuth client id.
    pub client_id: String,
    /// The OAuth client secret.
    pub client_secret: String,
    /// The redirect URI registered with the provider; the callback route.
    pub redirect_uri: String,
}

/// A persisted OAuth token set. Stored encrypted with the rest of a
/// connection's config; never returned in API responses. The tokens are masked
/// in [`Debug`] so a connection config's derived `Debug` cannot leak them.
#[derive(Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct OAuthTokens {
    /// The current access token, sent as a bearer credential.
    pub access_token: String,
    /// The refresh token, used to mint a new access token when it expires.
    /// Providers may omit it on refresh, so it is retained across refreshes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Unix seconds at which the access token expires, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

impl fmt::Debug for OAuthTokens {
    /// Masks the access and refresh tokens; only their presence and the expiry
    /// are shown.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthTokens")
            .field("access_token", &"<set>")
            .field(
                "refresh_token",
                match self.refresh_token {
                    Some(_) => &"<set>",
                    None => &"<unset>",
                },
            )
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl OAuthTokens {
    /// Whether the access token is expired (or within `skew_secs` of expiry).
    /// Tokens with no known expiry are treated as still valid.
    #[must_use]
    pub fn is_expired(&self, now_unix: i64, skew_secs: i64) -> bool {
        self.expires_at
            .is_some_and(|exp| now_unix + skew_secs >= exp)
    }
}

/// The outcome of starting an authorization: the URL to send the user to, and
/// the CSRF state and PKCE verifier to carry until the callback.
#[derive(Debug)]
pub struct Authorization {
    /// The provider authorize URL to redirect the user to.
    pub authorize_url: String,
    /// Opaque CSRF token; the callback must present the same value.
    pub csrf_state: String,
    /// PKCE verifier; the callback must present it to exchange the code.
    pub pkce_verifier: String,
}
