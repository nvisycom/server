//! Error types for Python interop.

use pyo3::PyErr;
use thiserror::Error;

use crate::error::{Error, ErrorKind};

/// Result type for Python interop operations.
pub type PyResult<T> = std::result::Result<T, PyError>;

/// Error type for Python interop operations.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct PyError {
    kind: PyErrorKind,
    message: String,
    #[source]
    source: Option<PyErr>,
}

#[derive(Debug, Clone, Copy)]
pub enum PyErrorKind {
    /// Failed to initialize Python interpreter.
    InitializationFailed,
    /// Failed to import the nvisy_dal module.
    ModuleNotFound,
    /// Provider not found in the Python package.
    ProviderNotFound,
    /// Failed to call a Python method.
    CallFailed,
    /// Type conversion error between Rust and Python.
    ConversionError,
}

impl PyError {
    pub fn new(kind: PyErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(mut self, source: PyErr) -> Self {
        self.source = Some(source);
        self
    }

    pub fn initialization(message: impl Into<String>) -> Self {
        Self::new(PyErrorKind::InitializationFailed, message)
    }

    pub fn module_not_found(message: impl Into<String>) -> Self {
        Self::new(PyErrorKind::ModuleNotFound, message)
    }

    pub fn provider_not_found(name: &str) -> Self {
        Self::new(
            PyErrorKind::ProviderNotFound,
            format!("Provider '{}' not found in nvisy_dal", name),
        )
    }

    pub fn call_failed(message: impl Into<String>) -> Self {
        Self::new(PyErrorKind::CallFailed, message)
    }

    pub fn conversion(message: impl Into<String>) -> Self {
        Self::new(PyErrorKind::ConversionError, message)
    }
}

impl From<PyErr> for PyError {
    fn from(err: PyErr) -> Self {
        Self::new(PyErrorKind::CallFailed, err.to_string()).with_source(err)
    }
}

impl From<PyError> for Error {
    fn from(err: PyError) -> Self {
        let kind = match err.kind {
            PyErrorKind::InitializationFailed | PyErrorKind::ModuleNotFound => {
                ErrorKind::Connection
            }
            PyErrorKind::ProviderNotFound => ErrorKind::NotFound,
            PyErrorKind::ConversionError => ErrorKind::InvalidInput,
            PyErrorKind::CallFailed => ErrorKind::Provider,
        };

        Error::new(kind, err.message)
    }
}
