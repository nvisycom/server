use clap::Args;
use serde::{Deserialize, Serialize};

/// App [`OpenApi`](utoipa::OpenApi) configuration.
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct OpenApiConfig {
    /// Path which exposes the OpenApi to the user.
    #[arg(short, long, default_value = "/api/openapi.json")]
    pub open_api_json: String,

    /// Path which exposes SwaggerUI to the user.
    #[arg(short, long, default_value = "/api/swagger")]
    pub swagger_ui: String,

    /// Path which exposes Scalar to the user.
    #[arg(short, long, default_value = "/api/scalar")]
    pub scalar_ui: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            open_api_json: "/api/openapi.json".to_owned(),
            swagger_ui: "/api/swagger".to_string(),
            scalar_ui: "/api/scalar".to_string(),
        }
    }
}
