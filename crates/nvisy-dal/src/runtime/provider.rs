//! Python provider wrapper implementing Rust traits.

use std::marker::PhantomData;

use async_stream::try_stream;
use futures::Stream;
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::types::PyAnyMethods;
use pyo3::{Py, PyAny, Python};

use super::PyError;
use super::loader::{json_to_pydict, json_to_pyobject, pyobject_to_json};
use crate::streams::InputStream;
use crate::{DataInput, DataOutput, Result, Resumable};

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
    Ctx: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
{
    type Context = Ctx;
    type Datatype = T;

    async fn read(
        &self,
        ctx: &Self::Context,
    ) -> Result<InputStream<Resumable<Self::Datatype, Self::Context>>> {
        let ctx_json = serde_json::to_value(ctx)
            .map_err(|e| PyError::conversion(format!("Failed to serialize context: {}", e)))?;

        // Call Python read method which returns an async iterator
        let coro = Python::attach(|py| {
            let bound = self.provider.instance.bind(py);
            let ctx_dict = json_to_pydict(py, &ctx_json)?;
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
        // Python yields (item, context) tuples, we convert to Resumable
        let stream = py_async_iterator_to_stream::<T, Ctx>(py_iterator);
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
    type Datatype = T;

    async fn write(&self, items: Vec<Self::Datatype>) -> Result<()> {
        let items_json = serde_json::to_value(&items)
            .map_err(|e| PyError::conversion(format!("Failed to serialize items: {}", e)))?;

        let coro = Python::attach(|py| {
            let bound = self.provider.instance.bind(py);
            let items_list = json_to_pyobject(py, &items_json)?;
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

/// Converts a Python async iterator to a Rust Stream of Resumable items.
///
/// Python yields `(data, context)` tuples which are converted to `Resumable<T, C>`.
fn py_async_iterator_to_stream<T, C>(
    iterator: Py<PyAny>,
) -> impl Stream<Item = Result<Resumable<T, C>>>
where
    T: for<'de> serde::Deserialize<'de> + Send + 'static,
    C: for<'de> serde::Deserialize<'de> + Send + 'static,
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
                        if e.is_instance_of::<PyStopAsyncIteration>(py) {
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

            // Convert Python (data, context) tuple to Resumable
            let json_value = Python::attach(|py| pyobject_to_json(result.bind(py)))?;
            let (data, context): (T, C) = serde_json::from_value(json_value)
                .map_err(|e| PyError::conversion(format!("Failed to deserialize item: {}", e)))?;

            yield Resumable::new(data, context);
        }
    }
}
