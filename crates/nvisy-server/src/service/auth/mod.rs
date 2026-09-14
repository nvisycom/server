//! Authentication subsystem: sign-in flows, session-token issuance and keys,
//! password handling, OIDC, and OIDC account provisioning.

mod account_provisioner;
mod auth_flow;
mod auth_issuer;
mod auth_keys;
mod oidc;
mod password;

pub use account_provisioner::AccountProvisioner;
pub use auth_flow::SignInService;
pub use auth_issuer::AuthIssuer;
pub use auth_keys::{AuthKeys, AuthKeysConfig};
pub use oidc::{
    CallbackOutcome, ConsumedFlow, OidcAuthorization, OidcConfig, OidcConfigured, OidcError,
    OidcIdentity, OidcPurpose, OidcService, RedirectKind,
};
pub use password::PasswordService;
