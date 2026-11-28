//! `OAuth2` authorization-code flow, driven through the shared `reqwest` client.
//!
//! Provides the reusable pieces every OAuth file-service provider needs: the
//! per-provider endpoint/scope description ([`OAuthProvider`]), the persisted
//! token set ([`OAuthTokens`]), and an [`OAuthClient`], built once per provider
//! and app, that builds an authorize URL with PKCE and CSRF state, and exchanges
//! codes and refresh tokens.
//!
//! The `oauth2` crate is used with no built-in HTTP client; every token request
//! runs through `nvisy-core`'s OAuth transport over the shared `reqwest` client,
//! so OAuth traffic shares the platform's TLS, retry, and tracing setup.

mod client;
mod types;

pub use self::client::OAuthClient;
pub use self::types::{Authorization, OAuthApp, OAuthProvider, OAuthTokens};
