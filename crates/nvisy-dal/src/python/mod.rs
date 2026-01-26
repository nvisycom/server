//! Python interop for data providers.
//!
//! This module provides integration with the `nvisy_dal` Python package,
//! allowing Rust code to load and use Python-based providers.

mod error;
mod loader;
mod provider;

pub(crate) use loader::PyProviderLoader;
pub(crate) use provider::{PyDataInput, PyDataOutput, PyProvider};
