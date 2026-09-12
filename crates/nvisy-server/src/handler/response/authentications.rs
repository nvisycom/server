//! Authentication response types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The result of minting a native-app (desktop) session token.
///
/// The frontend hands `apiToken` to the desktop app via the `redirectUri`
/// deep-link (`{redirectUri}?token={apiToken}`); the app stores it and sends it as
/// an `Authorization: Bearer` credential. Browser web sessions use cookies
/// instead and return no token.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountDesktopToken {
    /// The signed `app` JWT to send as a Bearer token.
    pub api_token: String,
    /// The desktop deep-link the token should be delivered on, echoed back.
    pub redirect_uri: String,
}
