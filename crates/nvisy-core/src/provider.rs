//! Provider creation trait.

use crate::Result;

/// Trait for creating a provider from parameters and credentials.
///
/// This trait bridges non-sensitive parameters (like bucket name, table, model)
/// with sensitive credentials (like API keys, secrets) to construct
/// a fully configured provider instance.
///
/// # Type Parameters
///
/// - `Params`: Non-sensitive configuration (e.g., bucket name, model name)
/// - `Credentials`: Sensitive authentication data (e.g., API keys, secrets)
///
/// # Example
///
/// ```ignore
/// #[async_trait::async_trait]
/// impl IntoProvider for S3Provider {
///     type Params = S3Params;
///     type Credentials = S3Credentials;
///
///     async fn create(params: Self::Params, credentials: Self::Credentials) -> Result<Self> {
///         // Build provider from params and credentials
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait IntoProvider: Send {
    /// Non-sensitive parameters (bucket, prefix, table, model, etc.).
    type Params: Send;
    /// Sensitive credentials (API keys, secrets, etc.).
    type Credentials: Send;

    /// Creates a new provider from parameters and credentials.
    async fn create(params: Self::Params, credentials: Self::Credentials) -> Result<Self>
    where
        Self: Sized;
}
