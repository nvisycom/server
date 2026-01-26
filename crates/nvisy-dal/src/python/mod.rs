//! Python interop for data providers.
//!
//! This module provides integration with the `nvisy_dal` Python package,
//! allowing Rust code to load and use Python-based providers.

mod error;
mod loader;
mod provider;

pub(crate) use error::{PyError, PyResult};
pub(crate) use loader::PyProviderLoader;
pub(crate) use provider::{PyDataInput, PyDataOutput, PyProvider};

/// Connects to a Python provider by name with the given credentials and parameters.
///
/// # Arguments
///
/// * `name` - Provider name (e.g., "postgres", "pinecone", "s3")
/// * `credentials` - Serializable credentials
/// * `params` - Serializable connection parameters
pub(crate) async fn connect<C, P>(
    name: &str,
    credentials: C,
    params: P,
) -> crate::Result<PyProvider>
where
    C: serde::Serialize,
    P: serde::Serialize,
{
    let loader = PyProviderLoader::new().map_err(crate::Error::from)?;
    let creds_json = serde_json::to_value(credentials).map_err(crate::Error::from)?;
    let params_json = serde_json::to_value(params).map_err(crate::Error::from)?;
    loader
        .load(name, creds_json, params_json)
        .await
        .map_err(crate::Error::from)
}
