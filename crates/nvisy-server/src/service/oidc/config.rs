//! Deployment configuration for OIDC sign-in providers.
//!
//! One [`clap::Args`]-derived struct per provider (behind the `cli` feature),
//! mirroring the cloud file-service OAuth config. A provider whose required
//! values are not all set is left unconfigured and cannot be used to sign in.

use nvisy_postgres::types::IdentityProvider;

/// A resolved OIDC provider: the credentials and issuer needed to run a sign-in.
///
/// Produced from a provider's config only when every required value is present,
/// so a partially-configured provider is simply absent rather than half-usable.
#[derive(Debug, Clone)]
pub struct ResolvedOidcProvider {
    /// Which identity provider this is.
    pub provider: IdentityProvider,
    /// OIDC client id.
    pub client_id: String,
    /// OIDC client secret.
    pub client_secret: String,
    /// The sign-in callback URL registered with the provider.
    pub redirect_uri: String,
    /// OIDC issuer URL used for discovery.
    pub issuer: String,
}

/// Defines one provider's OIDC config struct.
///
/// Each provider gets its own struct (rather than one flattened twice) because
/// clap-derive cannot prefix a flattened struct's args, so the flags — and their
/// `long`/`env` — are declared per provider to avoid collisions when both are
/// flattened into [`OidcConfig`].
macro_rules! oidc_provider_config {
    (
        $(#[$meta:meta])*
        $name:ident,
        provider = $provider:expr,
        default_issuer = $default_issuer:literal,
        $id_long:literal, $id_env:literal,
        $secret_long:literal, $secret_env:literal,
        $redirect_long:literal, $redirect_env:literal,
        $issuer_long:literal, $issuer_env:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default)]
        #[cfg_attr(feature = "cli", derive(clap::Args))]
        pub struct $name {
            /// OIDC client id.
            #[cfg_attr(feature = "cli", arg(id = $id_long, long = $id_long, env = $id_env))]
            pub client_id: Option<String>,
            /// OIDC client secret.
            #[cfg_attr(
                feature = "cli",
                arg(id = $secret_long, long = $secret_long, env = $secret_env)
            )]
            pub client_secret: Option<String>,
            /// OIDC redirect URI (the sign-in callback registered with the provider).
            #[cfg_attr(
                feature = "cli",
                arg(id = $redirect_long, long = $redirect_long, env = $redirect_env)
            )]
            pub redirect_uri: Option<String>,
            /// OIDC issuer URL (for discovery). Defaults per provider when unset.
            #[cfg_attr(
                feature = "cli",
                arg(id = $issuer_long, long = $issuer_long, env = $issuer_env)
            )]
            pub issuer: Option<String>,
        }

        impl $name {
            /// The default issuer used when [`issuer`](Self::issuer) is unset.
            pub const DEFAULT_ISSUER: &'static str = $default_issuer;

            /// Resolves to a [`ResolvedOidcProvider`], present only when the client
            /// id, secret, and redirect URI are all set to non-empty values. A
            /// blank value (e.g. a bare `GOOGLE_CLIENT_ID=` line) counts as unset.
            /// The issuer falls back to [`DEFAULT_ISSUER`](Self::DEFAULT_ISSUER).
            fn resolve(&self) -> Option<ResolvedOidcProvider> {
                let non_empty = |value: &Option<String>| {
                    value
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                };
                Some(ResolvedOidcProvider {
                    provider: $provider,
                    client_id: non_empty(&self.client_id)?,
                    client_secret: non_empty(&self.client_secret)?,
                    redirect_uri: non_empty(&self.redirect_uri)?,
                    issuer: non_empty(&self.issuer)
                        .unwrap_or_else(|| Self::DEFAULT_ISSUER.to_owned()),
                })
            }
        }
    };
}

oidc_provider_config!(
    /// Google sign-in credentials.
    GoogleOidcConfig,
    provider = IdentityProvider::Google,
    default_issuer = "https://accounts.google.com",
    "google-client-id",
    "GOOGLE_CLIENT_ID",
    "google-client-secret",
    "GOOGLE_CLIENT_SECRET",
    "google-redirect-uri",
    "GOOGLE_REDIRECT_URI",
    "google-issuer",
    "GOOGLE_ISSUER"
);

oidc_provider_config!(
    /// Microsoft (Entra) sign-in credentials.
    ///
    /// The default issuer is the multi-tenant `common` endpoint, which admits any
    /// Microsoft work, school, or personal account. A single-organization
    /// deployment can pin its tenant by setting the issuer to
    /// `https://login.microsoftonline.com/{tenant-id}/v2.0`.
    MicrosoftOidcConfig,
    provider = IdentityProvider::Microsoft,
    default_issuer = "https://login.microsoftonline.com/common/v2.0",
    "microsoft-client-id",
    "MICROSOFT_CLIENT_ID",
    "microsoft-client-secret",
    "MICROSOFT_CLIENT_SECRET",
    "microsoft-redirect-uri",
    "MICROSOFT_REDIRECT_URI",
    "microsoft-issuer",
    "MICROSOFT_ISSUER"
);

/// Deployment configuration for OIDC sign-in: one struct per supported provider.
///
/// Each provider is independently optional; a deployment enables Google, or
/// Microsoft, or both, or neither, by supplying the relevant credentials.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct OidcConfig {
    /// Google sign-in credentials.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub google: GoogleOidcConfig,
    /// Microsoft sign-in credentials.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub microsoft: MicrosoftOidcConfig,
    /// Allowed frontend origins the sign-in callback may redirect to, as a
    /// comma-separated list of `scheme://host[:port]` origins.
    ///
    /// The callback returns the browser to a caller-supplied `redirectUri`; that
    /// URL must match one of these origins or the redirect is refused, so a
    /// caller cannot point the flow (which carries the minted session token) at an
    /// arbitrary attacker host. Empty means no external redirect is allowed — the
    /// callback then renders a minimal in-page result instead.
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "oidc-allowed-redirect-origins",
            env = "OIDC_ALLOWED_REDIRECT_ORIGINS",
            value_delimiter = ',',
            num_args = 0..,
        )
    )]
    pub allowed_redirect_origins: Vec<String>,
}

impl OidcConfig {
    /// The providers that are fully configured and therefore usable for sign-in.
    #[must_use]
    pub fn resolved_providers(&self) -> Vec<ResolvedOidcProvider> {
        [self.google.resolve(), self.microsoft.resolve()]
            .into_iter()
            .flatten()
            .collect()
    }

    /// The configured allowed redirect origins, trimmed and lowercased, with
    /// blanks dropped.
    #[must_use]
    pub fn allowed_redirect_origins(&self) -> Vec<String> {
        self.allowed_redirect_origins
            .iter()
            .map(|origin| origin.trim().to_ascii_lowercase())
            .filter(|origin| !origin.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_resolves_only_when_all_required_values_set() {
        let config = OidcConfig {
            // Fully set -> resolves, issuer defaulted.
            google: GoogleOidcConfig {
                client_id: Some("id".to_owned()),
                client_secret: Some("secret".to_owned()),
                redirect_uri: Some("https://app/auth/google/callback".to_owned()),
                issuer: None,
            },
            // Missing secret -> unset.
            microsoft: MicrosoftOidcConfig {
                client_id: Some("id".to_owned()),
                client_secret: Some("  ".to_owned()),
                redirect_uri: Some("https://app/auth/microsoft/callback".to_owned()),
                issuer: None,
            },
            ..Default::default()
        };

        let resolved = config.resolved_providers();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].provider, IdentityProvider::Google);
        assert_eq!(resolved[0].issuer, GoogleOidcConfig::DEFAULT_ISSUER);
    }

    #[test]
    fn issuer_override_is_respected() {
        let config = MicrosoftOidcConfig {
            client_id: Some("id".to_owned()),
            client_secret: Some("secret".to_owned()),
            redirect_uri: Some("https://app/auth/microsoft/callback".to_owned()),
            issuer: Some("https://login.microsoftonline.com/tenant-123/v2.0".to_owned()),
        };
        let resolved = config.resolve().expect("configured");
        assert_eq!(
            resolved.issuer,
            "https://login.microsoftonline.com/tenant-123/v2.0"
        );
    }
}
