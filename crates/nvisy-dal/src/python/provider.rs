//! Python provider wrapper implementing Rust traits.

use std::marker::PhantomData;

use async_stream::try_stream;
use futures::Stream;
use pyo3::prelude::*;

use super::PyError;
use super::loader::pyobject_to_json;
use crate::Result;
use crate::core::{DataInput, DataOutput, InputStream};

/// A wrapper around a Python provider instance.
///
/// Implements the Rust `DataInput` and `DataOutput` traits by delegating
/// to the underlying Python provider's `read` and `write` methods.
pub struct PyProvider {
    instance: Py<PyAny>,
}

impl PyProvider {
    /// Creates a new PyProvider from a connected Python provider instance.
    pub fn new(instance: Py<PyAny>) -> Self {
        Self { instance }
    }

    /// Clones the underlying Python object reference.
    pub fn clone_py_object(&self) -> Py<PyAny> {
        Python::attach(|py| self.instance.clone_ref(py))
    }

    /// Creates a typed `DataInput` wrapper from this provider.
    pub fn as_data_input<T, Ctx>(&self) -> PyDataInput<T, Ctx> {
        PyDataInput::new(Self::new(self.clone_py_object()))
    }

    /// Creates a typed `DataOutput` wrapper from this provider.
    pub fn as_data_output<T>(&self) -> PyDataOutput<T> {
        PyDataOutput::new(Self::new(self.clone_py_object()))
    }

    /// Disconnects the provider.
    pub async fn disconnect(&self) -> Result<()> {
        let coro = Python::attach(|py| {
            let coro = self
                .instance
                .bind(py)
                .call_method0("disconnect")
                .map_err(|e| PyError::call_failed(format!("Failed to call disconnect: {}", e)))?;
            pyo3_async_runtimes::tokio::into_future(coro)
                .map_err(|e| PyError::call_failed(format!("Failed to convert to future: {}", e)))
        })?;

        coro.await
            .map_err(|e| PyError::call_failed(format!("Failed to disconnect: {}", e)))?;

        Ok(())
    }
}

impl std::fmt::Debug for PyProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyProvider").finish_non_exhaustive()
    }
}

/// Typed wrapper for Python providers implementing DataInput.
pub struct PyDataInput<T, Ctx> {
    provider: PyProvider,
    _marker: PhantomData<(T, Ctx)>,
}

impl<T, Ctx> PyDataInput<T, Ctx> {
    /// Creates a new typed input wrapper.
    pub fn new(provider: PyProvider) -> Self {
        Self {
            provider,
            _marker: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<T, Ctx> DataInput for PyDataInput<T, Ctx>
where
    T: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    Ctx: serde::Serialize + Send + Sync,
{
    type Context = Ctx;
    type Item = T;

    async fn read(&self, ctx: &Self::Context) -> Result<InputStream<Self::Item>> {
        let ctx_json = serde_json::to_value(ctx)
            .map_err(|e| PyError::conversion(format!("Failed to serialize context: {}", e)))?;

        // Call Python read method which returns an async iterator
        let coro = Python::attach(|py| {
            let bound = self.provider.instance.bind(py);
            let ctx_dict = super::loader::json_to_pydict(py, &ctx_json)?;
            let coro = bound
                .call_method1("read", (ctx_dict,))
                .map_err(|e| PyError::call_failed(format!("Failed to call read: {}", e)))?;
            pyo3_async_runtimes::tokio::into_future(coro)
                .map_err(|e| PyError::call_failed(format!("Failed to convert to future: {}", e)))
        })?;

        let py_iterator = coro
            .await
            .map_err(|e| PyError::call_failed(format!("Failed to call read: {}", e)))?;

        // Create a stream that pulls from the Python async iterator
        let stream = py_async_iterator_to_stream::<T>(py_iterator);
        Ok(InputStream::new(Box::pin(stream)))
    }
}

/// Typed wrapper for Python providers implementing DataOutput.
pub struct PyDataOutput<T> {
    provider: PyProvider,
    _marker: PhantomData<T>,
}

impl<T> PyDataOutput<T> {
    /// Creates a new typed output wrapper.
    pub fn new(provider: PyProvider) -> Self {
        Self {
            provider,
            _marker: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<T> DataOutput for PyDataOutput<T>
where
    T: serde::Serialize + Send + Sync,
{
    type Item = T;

    async fn write(&self, items: Vec<Self::Item>) -> Result<()> {
        let items_json = serde_json::to_value(&items)
            .map_err(|e| PyError::conversion(format!("Failed to serialize items: {}", e)))?;

        let coro = Python::attach(|py| {
            let bound = self.provider.instance.bind(py);
            let items_list = super::loader::json_to_pyobject(py, &items_json)?;
            let coro = bound
                .call_method1("write", (items_list,))
                .map_err(|e| PyError::call_failed(format!("Failed to call write: {}", e)))?;
            pyo3_async_runtimes::tokio::into_future(coro)
                .map_err(|e| PyError::call_failed(format!("Failed to convert to future: {}", e)))
        })?;

        coro.await
            .map_err(|e| PyError::call_failed(format!("Failed to call write: {}", e)))?;

        Ok(())
    }
}

/// Converts a Python async iterator to a Rust Stream.
fn py_async_iterator_to_stream<T>(iterator: Py<PyAny>) -> impl Stream<Item = Result<T>>
where
    T: for<'de> serde::Deserialize<'de> + Send + 'static,
{
    try_stream! {
        loop {
            // Get the next coroutine from __anext__
            let next_coro = Python::attach(|py| {
                let bound = iterator.bind(py);
                match bound.call_method0("__anext__") {
                    Ok(coro) => {
                        let future = pyo3_async_runtimes::tokio::into_future(coro)?;
                        Ok(Some(future))
                    }
                    Err(e) => {
                        if e.is_instance_of::<pyo3::exceptions::PyStopAsyncIteration>(py) {
                            Ok(None)
                        } else {
                            Err(PyError::from(e))
                        }
                    }
                }
            })?;

            let Some(coro) = next_coro else {
                break;
            };

            // Await the coroutine
            let result = coro.await.map_err(PyError::from)?;

            // Convert result to Rust type
            let json_value = Python::attach(|py| pyobject_to_json(result.bind(py)))?;
            let item: T = serde_json::from_value(json_value)
                .map_err(|e| PyError::conversion(format!("Failed to deserialize item: {}", e)))?;

            yield item;
        }
    }
}
