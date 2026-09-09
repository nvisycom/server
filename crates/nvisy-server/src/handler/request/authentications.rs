//! Authentication request types.

use garde::Validate;
use nvisy_postgres::types::Handle;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Request payload for login.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct Login {
    /// Email address or username of the account.
    #[garde(length(chars, min = 3, max = 254))]
    pub identifier: String,
    /// Password of the account.
    #[garde(length(chars, min = 1, max = 1000))]
    pub password: String,
    /// Whether to remember this device for extended session. Defaults to false.
    #[serde(default)]
    pub remember_me: bool,
}

/// Request payload for signup.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct Signup {
    /// Public account handle, unique across all accounts.
    pub username: Handle,

    /// Optional display name of the account.
    #[garde(length(chars, min = 2, max = 32))]
    pub display_name: Option<String>,

    /// Email address of the account.
    #[garde(email, length(chars, min = 5, max = 254))]
    pub email_address: String,

    /// Password of the account.
    #[garde(length(chars, min = 8, max = 128))]
    pub password: String,

    /// Whether to remember the device for extended session. Defaults to false.
    #[serde(default)]
    pub remember_me: bool,
}

// TODO: Implement password reset

/// Request payload for password reset initiation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct RequestPasswordReset {
    /// Email address of the account to reset password for.
    #[garde(email, length(chars, min = 5, max = 254))]
    pub email_address: String,
}

/// Request payload for password reset confirmation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct ConfirmPasswordReset {
    /// Password reset token.
    #[garde(length(chars, min = 10, max = 200))]
    pub token: String,

    /// New password.
    #[garde(length(chars, min = 8, max = 128))]
    pub new_password: String,
}

/// Request payload to mint a native-app (desktop) session token.
///
/// Called by the frontend after a normal browser (cookie) login when the login
/// was initiated by the desktop app: it exchanges the just-established session for
/// a long-lived `app` token the frontend then hands to the app via the
/// `redirectUri` deep-link. The `redirectUri` must be a registered desktop scheme
/// (e.g. `nvisy://…`), so a token cannot be minted toward a web origin.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct DesktopTokenRequest {
    /// The desktop deep-link the app will receive the token on. Must match a
    /// configured desktop redirect scheme.
    #[garde(length(chars, min = 1, max = 2048))]
    pub redirect_uri: String,
}

/// Query parameters the provider appends when redirecting to the OIDC callback.
///
/// The callback serves every OIDC flow (sign-in, link, and step-up reauth); the
/// stashed state selects which. On success the provider sends `code` + `state`;
/// on denial it sends `error` (and `state`) with no `code`, so `code` is optional
/// and the handler treats a missing code or a present error as a failed flow.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcCallbackQuery {
    /// The authorization code to exchange for tokens; absent on a denial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The opaque CSRF state echoed back; must match a pending flow.
    pub state: String,
    /// The provider's error code when the user denied or the flow failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
