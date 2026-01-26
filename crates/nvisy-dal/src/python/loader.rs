//! Python package loader for nvisy_dal providers.

use std::sync::OnceLock;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use super::error::{PyError, PyResult};
use super::provider::PyProvider;

/// Global reference to the nvisy_dal Python module.
static NVISY_DAL_MODULE: OnceLock<Py<PyModule>> = OnceLock::new();

/// Loader for Python-based data providers.
///
/// Handles initialization of the Python interpreter and loading
/// of provider classes from the `nvisy_dal` package.
#[derive(Debug)]
pub struct PyProviderLoader {
    _private: (),
}

impl PyProviderLoader {
    /// Creates a new provider loader.
    ///
    /// Initializes the Python interpreter if not already done.
    pub fn new() -> PyResult<Self> {
        // Ensure Python is initialized (pyo3 auto-initialize feature handles this)
        Self::ensure_module_loaded()?;
        Ok(Self { _private: () })
    }

    /// Ensures the nvisy_dal module is loaded and cached.
    fn ensure_module_loaded() -> PyResult<()> {
        if NVISY_DAL_MODULE.get().is_some() {
            return Ok(());
        }

        Python::attach(|py| {
            let module = py.import("nvisy_dal").map_err(|e| {
                PyError::module_not_found("Failed to import nvisy_dal").with_source(e)
            })?;

            // Store a reference to the module
            let _ = NVISY_DAL_MODULE.set(module.unbind());
            Ok(())
        })
    }

    /// Loads a provider by name and connects with the given credentials.
    ///
    /// # Arguments
    ///
    /// * `name` - Provider name (e.g., "qdrant", "pinecone", "s3")
    /// * `credentials` - JSON-serializable credentials
    /// * `params` - JSON-serializable connection parameters
    pub async fn load(
        &self,
        name: &str,
        credentials: serde_json::Value,
        params: serde_json::Value,
    ) -> PyResult<PyProvider> {
        let name = name.to_owned();

        // Get the provider class and prepare arguments
        let (provider_class, creds_dict, params_dict) = Python::attach(|py| {
            let module = self.get_module(py)?;

            // Import the specific provider module
            let providers_mod = module
                .getattr("providers")
                .map_err(|e| PyError::module_not_found("providers").with_source(e))?;
            let provider_mod = providers_mod
                .getattr(name.as_str())
                .map_err(|e| PyError::provider_not_found(&name).with_source(e))?;

            // Get the Provider class
            let provider_class = provider_mod
                .getattr("Provider")
                .map_err(|e| PyError::provider_not_found(&name).with_source(e))?;

            // Convert credentials and params to Python dicts
            let creds_dict = json_to_pydict(py, &credentials)?;
            let params_dict = json_to_pydict(py, &params)?;

            Ok::<_, PyError>((
                provider_class.unbind(),
                creds_dict.unbind(),
                params_dict.unbind(),
            ))
        })?;

        // Call the async connect method
        let coro = Python::attach(|py| {
            let provider_class = provider_class.bind(py);
            let creds = creds_dict.bind(py);
            let params = params_dict.bind(py);

            let coro = provider_class.call_method1("connect", (creds, params))?;
            pyo3_async_runtimes::tokio::into_future(coro)
        })?;

        let instance = coro.await.map_err(PyError::from)?;

        Ok(PyProvider::new(instance))
    }

    fn get_module<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyModule>> {
        NVISY_DAL_MODULE
            .get()
            .map(|m| m.bind(py).clone())
            .ok_or_else(|| PyError::module_not_found("nvisy_dal module not loaded"))
    }
}

impl Default for PyProviderLoader {
    fn default() -> Self {
        Self::new().expect("Failed to initialize PyProviderLoader")
    }
}

/// Converts a serde_json::Value to a Python dict.
pub(super) fn json_to_pydict<'py>(
    py: Python<'py>,
    value: &serde_json::Value,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);

    if let serde_json::Value::Object(map) = value {
        for (key, val) in map {
            let py_val = json_to_pyobject(py, val)?;
            dict.set_item(key, py_val)
                .map_err(|e| PyError::conversion("Failed to set dict item").with_source(e))?;
        }
    }

    Ok(dict)
}

/// Converts a serde_json::Value to a Python object.
pub(super) fn json_to_pyobject<'py>(
    py: Python<'py>,
    value: &serde_json::Value,
) -> PyResult<Bound<'py, PyAny>> {
    let obj: Bound<'py, PyAny> = match value {
        serde_json::Value::Null => py.None().into_bound(py),
        serde_json::Value::Bool(b) => (*b)
            .into_pyobject(py)
            .expect("infallible")
            .to_owned()
            .into_any(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)
                    .expect("infallible")
                    .to_owned()
                    .into_any()
            } else if let Some(f) = n.as_f64() {
                f.into_pyobject(py)
                    .expect("infallible")
                    .to_owned()
                    .into_any()
            } else {
                return Err(PyError::conversion("Unsupported number type"));
            }
        }
        serde_json::Value::String(s) => {
            s.as_str().into_pyobject(py).expect("infallible").into_any()
        }
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                let py_item = json_to_pyobject(py, item)?;
                list.append(py_item)
                    .map_err(|e| PyError::conversion("Failed to append to list").with_source(e))?;
            }
            list.into_any()
        }
        serde_json::Value::Object(_) => json_to_pydict(py, value)?.into_any(),
    };

    Ok(obj)
}

/// Converts a Python object to a serde_json::Value.
pub(super) fn pyobject_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        return Ok(serde_json::Value::Null);
    }

    if let Ok(b) = obj.extract::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }

    if let Ok(i) = obj.extract::<i64>() {
        return Ok(serde_json::json!(i));
    }

    if let Ok(f) = obj.extract::<f64>() {
        return Ok(serde_json::json!(f));
    }

    if let Ok(s) = obj.extract::<String>() {
        return Ok(serde_json::Value::String(s));
    }

    if let Ok(list) = obj.cast::<PyList>() {
        let mut arr = Vec::new();
        for item in list.iter() {
            arr.push(pyobject_to_json(item.as_any())?);
        }
        return Ok(serde_json::Value::Array(arr));
    }

    if let Ok(dict) = obj.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key_str: String = key
                .extract()
                .map_err(|e| PyError::conversion("Dict key must be string").with_source(e))?;
            map.insert(key_str, pyobject_to_json(&value)?);
        }
        return Ok(serde_json::Value::Object(map));
    }

    let type_name = obj
        .get_type()
        .name()
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    Err(PyError::conversion(format!(
        "Unsupported Python type: {}",
        type_name
    )))
}
