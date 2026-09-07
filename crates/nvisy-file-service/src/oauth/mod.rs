//! OAuth2 authorization-code flow, driven through the shared `reqwest` client.
//!
//! Provides the reusable pieces every OAuth file-service provider needs: the
//! per-provider endpoint/scope description ([`OAuthProvider`]), the persisted
//! token set ([`OAuthTokens`]), and an [`OAuthClient`] bound to a provider, app,
//! and HTTP client that builds an authorize URL with PKCE and CSRF state, and
//! exchanges codes and refresh tokens.
//!
//! The `oauth2` crate is used with no built-in HTTP client; every token request
//! runs through an adapter over a caller-supplied [`reqwest::Client`], so OAuth
//! traffic shares the same TLS/proxy/tracing setup as the rest of the platform.

mod client;
mod http_client;
mod types;

pub use self::client::OAuthClient;
pub use self::types::{Authorization, OAuthApp, OAuthProvider, OAuthTokens};
