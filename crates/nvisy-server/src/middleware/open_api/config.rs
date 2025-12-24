use serde::{Deserialize, Serialize};

/// App OpenAPI configuration for aide.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct OpenApiConfig {
    /// Path which exposes the OpenApi to the user.
    pub open_api_json: String,

    /// Path which exposes Scalar to the user.
    pub scalar_ui: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            open_api_json: "/api/openapi.json".to_owned(),
            scalar_ui: "/api/scalar".to_string(),
        }
    }
}
