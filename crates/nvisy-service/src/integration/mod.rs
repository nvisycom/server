//! This module provides the integration registry and provider definitions
//! for connecting workspaces to external services.

mod provider;

pub use provider::{
    IntegrationAuthType, IntegrationCategory, IntegrationDirection, IntegrationProvider,
    OAuthConfig,
};
