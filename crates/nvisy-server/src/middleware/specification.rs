//! OpenAPI specification middleware with Scalar UI integration.
//!
//! This module provides OpenAPI documentation generation and serving capabilities
//! using the [`aide`] crate with Scalar UI for interactive API exploration.
//!
//! # Overview
//!
//! The specification module offers:
//! - Automatic OpenAPI spec generation from aide's [`ApiRouter`]
//! - Scalar UI for interactive API documentation
//! - Configurable paths for JSON spec and UI endpoints
//!
//! # Usage
//!
//! ```rust
//! use aide::axum::ApiRouter;
//! use axum::Router;
//! use nvisy_server::middleware::{OpenApiConfig, RouterOpenApiExt};
//!
//! let app: Router<()> = ApiRouter::new()
//!     .with_open_api(&OpenApiConfig::default());
//! ```
//!
//! [`aide`]: https://docs.rs/aide
//! [`ApiRouter`]: aide::axum::ApiRouter

use aide::axum::ApiRouter;
use aide::openapi::{Contact, License, OpenApi, Tag};
use aide::scalar::Scalar;
use aide::transform::TransformOpenApi;
use axum::routing::{Router, get};
use axum::{Extension, Json};
use serde_json::Value;

/// OpenAPI configuration for aide integration.
///
/// Configures the paths where the OpenAPI JSON specification and
/// Scalar UI will be served.
#[derive(Debug, Clone)]
#[must_use = "config does nothing unless you use it"]
pub struct OpenApiConfig {
    /// Path which exposes the OpenAPI JSON specification.
    pub open_api_json: String,

    /// Path which exposes the Scalar API reference UI.
    pub scalar_ui: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            open_api_json: "/api/openapi.json".to_owned(),
            scalar_ui: "/api/scalar".to_owned(),
        }
    }
}

/// Extension trait for [`ApiRouter`] to add OpenAPI documentation with Scalar UI.
///
/// This trait provides convenient methods to generate and serve OpenAPI documentation
/// from your aide-annotated routes.
///
/// [`ApiRouter`]: aide::axum::ApiRouter
pub trait RouterOpenApiExt<S> {
    /// Adds OpenAPI documentation routes.
    ///
    /// This method:
    /// - Generates the OpenAPI specification from the router's API routes
    /// - Adds a route to serve the OpenAPI JSON specification
    /// - Adds a route to serve the Scalar API reference UI
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for OpenAPI and Scalar UI paths
    ///
    /// # Example
    ///
    /// ```rust
    /// use aide::axum::ApiRouter;
    /// use axum::Router;
    /// use nvisy_server::middleware::{OpenApiConfig, RouterOpenApiExt};
    ///
    /// let app: Router<()> = ApiRouter::new()
    ///     .with_open_api(&OpenApiConfig::default());
    /// ```
    fn with_open_api(self, config: &OpenApiConfig) -> Router<S>;
}

impl<S> RouterOpenApiExt<S> for ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_open_api(self, config: &OpenApiConfig) -> Router<S> {
        async fn serve_openapi(Extension(api): Extension<OpenApi>) -> Json<OpenApi> {
            Json(api)
        }

        let mut api = OpenApi::default();

        // Add Scalar UI route and OpenAPI JSON route
        let scalar = Scalar::new(&config.open_api_json);
        let router = self
            .route(&config.scalar_ui, scalar.axum_route())
            .route(&config.open_api_json, get(serve_openapi));

        let router = router.finish_api_with(&mut api, api_docs);
        collapse_null_types(&mut api);
        router.layer(Extension(api))
    }
}

/// Removes the `null` variant that schemars adds to optional-field schemas
/// across every component schema.
///
/// schemars encodes `Option<T>` in one of two ways: for primitives as the union
/// `type: [T, null]`, and for referenced types (enums, structs) as
/// `anyOf: [T, { type: null }]`. Both rely on absence from `required` to signal
/// optionality. Optional response fields are omitted when absent rather than
/// serialized as `null` (each carries `skip_serializing_if = "Option::is_none"`),
/// so the `null` variant describes a value the API never emits and only widens
/// generated clients to `T | null`. Dropping it tightens each schema to match
/// what is actually sent.
///
/// This runs after schema generation because aide accumulates the component
/// schemas into `api.components` only once the router is finished.
fn collapse_null_types(api: &mut OpenApi) {
    let Some(components) = api.components.as_mut() else {
        return;
    };

    for schema in components.schemas.values_mut() {
        if let Some(object) = schema.json_schema.as_object_mut() {
            collapse_in_object(object);
        }
    }
}

/// Recursively removes null variants from optional-field schemas within a value.
fn collapse_in_value(value: &mut Value) {
    match value {
        Value::Object(map) => collapse_in_object(map),
        Value::Array(items) => items.iter_mut().for_each(collapse_in_value),
        _ => {}
    }
}

/// Removes the `null` variant a schema object gained from an `Option<T>`, in
/// both the `type: [T, null]` and `anyOf: [T, { type: null }]` forms, then
/// recurses into nested schemas.
///
/// A `type` array that collapses to a single member becomes that member as a
/// plain string. An `anyOf` whose only remaining branch is a single schema is
/// hoisted into this object, preserving sibling keywords such as `description`.
fn collapse_in_object(map: &mut serde_json::Map<String, Value>) {
    if let Some(Value::Array(types)) = map.get_mut("type") {
        types.retain(|entry| entry != "null");
        if let [only] = types.as_slice() {
            let only = only.clone();
            map.insert("type".to_owned(), only);
        }
    }

    if let Some(Value::Array(variants)) = map.get_mut("anyOf") {
        variants.retain(|variant| variant.get("type") != Some(&Value::String("null".to_owned())));
        if let [Value::Object(only)] = variants.as_slice() {
            let only = only.clone();
            map.remove("anyOf");
            for (key, value) in only {
                map.entry(key).or_insert(value);
            }
        }
    }

    for nested in map.values_mut() {
        collapse_in_value(nested);
    }
}

/// Transforms the OpenAPI specification with info and tags.
///
/// This function configures the OpenAPI documentation with API info and
/// organized tags for different API sections.
fn api_docs(api: TransformOpenApi) -> TransformOpenApi {
    api.title("Nvisy API")
        .summary("Document processing and annotation platform")
        .description(
            "Nvisy provides intelligent document processing, annotation, and analysis \
            capabilities. This API enables document upload, OCR processing, embedding \
            generation, and semantic search across your document collections.",
        )
        .version(env!("CARGO_PKG_VERSION"))
        .tos("https://nvisy.com/legal/terms-of-service")
        .contact(Contact {
            name: Some("Nvisy Support".to_owned()),
            url: Some("https://nvisy.com".to_owned()),
            email: Some("hello@nvisy.com".to_owned()),
            ..Contact::default()
        })
        .license(License {
            name: "Proprietary".to_owned(),
            url: Some("https://nvisy.com/license".to_owned()),
            ..License::default()
        })
        .tag(Tag {
            name: "Accounts".into(),
            description: Some("Account management and profile operations".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Authentication".into(),
            description: Some("Login, signup, and token management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Workspaces".into(),
            description: Some("Workspace creation and management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Files".into(),
            description: Some("File upload, download, and management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Members".into(),
            description: Some("Workspace member management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Invites".into(),
            description: Some("Workspace invitation handling".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Tokens".into(),
            description: Some("API token management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Connections".into(),
            description: Some("External provider connections".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Webhooks".into(),
            description: Some("Webhook configuration".into()),
            ..Default::default()
        })
}

#[cfg(test)]
mod tests {
    use aide::openapi::{Components, OpenApi, SchemaObject};
    use serde_json::json;

    use super::collapse_null_types;

    #[test]
    fn collapses_null_unions_across_nested_schemas() {
        let mut api = OpenApi::default();
        let mut components = Components::default();
        components.schemas.insert(
            "Sample".to_owned(),
            SchemaObject {
                json_schema: json!({
                    "type": "object",
                    "properties": {
                        "required": { "type": "string" },
                        "optional": { "type": ["string", "null"] },
                        "enumeration": {
                            "description": "field description",
                            "anyOf": [
                                { "$ref": "#/components/schemas/SortOrder" },
                                { "type": "null" }
                            ]
                        },
                        "nested": {
                            "type": "object",
                            "properties": {
                                "count": { "type": ["integer", "null"] }
                            }
                        }
                    },
                    "required": ["required"]
                })
                .try_into()
                .expect("valid schema"),
                external_docs: None,
                example: None,
            },
        );
        api.components = Some(components);

        collapse_null_types(&mut api);

        let schema = api.components.as_ref().unwrap().schemas["Sample"]
            .json_schema
            .as_value();

        assert_eq!(
            schema.pointer("/properties/optional/type"),
            Some(&json!("string")),
            "optional primitive collapses to a plain string"
        );
        assert_eq!(
            schema.pointer("/properties/enumeration/$ref"),
            Some(&json!("#/components/schemas/SortOrder")),
            "optional referenced type hoists the non-null anyOf branch"
        );
        assert!(
            schema.pointer("/properties/enumeration/anyOf").is_none(),
            "the anyOf wrapper is removed once null is stripped"
        );
        assert_eq!(
            schema.pointer("/properties/enumeration/description"),
            Some(&json!("field description")),
            "the field's own description survives hoisting"
        );
        assert_eq!(
            schema.pointer("/properties/nested/properties/count/type"),
            Some(&json!("integer")),
            "null is stripped from nested schemas too"
        );
        assert_eq!(
            schema.pointer("/properties/required/type"),
            Some(&json!("string")),
            "non-nullable fields are unaffected"
        );
    }

    /// Guards the invariant [`collapse_null_types`] relies on: because the spec
    /// strips `null` from every optional field's type, a response field that
    /// serialized `null` would make the schema lie. Every `Option<T>` field in
    /// a response type must therefore either omit itself when `None`
    /// (`skip_serializing_if`) or never serialize at all (`skip`).
    #[test]
    fn response_optionals_never_serialize_null() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/handler/response");

        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(dir).expect("response dir is readable") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }

            let source = std::fs::read_to_string(&path).expect("source is readable");
            let lines: Vec<&str> = source.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                let is_optional_field = trimmed.starts_with("pub ")
                    && !trimmed.starts_with("pub fn ")
                    && trimmed.contains(": Option<")
                    && !trimmed.contains("//");
                if !is_optional_field {
                    continue;
                }

                let start = index.saturating_sub(4);
                let context = lines[start..index].join("\n");
                let skipped = context.contains("skip_serializing_if")
                    || context.contains("serde(skip)")
                    || context.contains("serde(skip,")
                    || context.contains("serde(skip ");
                if !skipped {
                    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    offenders.push(format!("{file}:{}  {}", index + 1, trimmed));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "response Option<T> fields missing `skip_serializing_if`/`skip` \
             (they would serialize null, which the OpenAPI schema strips):\n{}",
            offenders.join("\n")
        );
    }
}
