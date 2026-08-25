//! Deployment catalog: read-only reference data describing what this deployment
//! offers, independent of any workspace.
//!
//! Exposes the label taxonomy (the categories of sensitive data policies can
//! target) and the recognizers the engine has registered. Both are
//! deployment-owned reference data, not persisted rows: labels come from the
//! runtime's built-in [`LabelCatalog`], recognizers from the configured
//! [`Engine`](elide_pipeline::Engine) lineup.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use elide_pipeline::entity::LabelCatalog;

use crate::extract::{AuthState, Json};
use crate::handler::response::{ErrorResponse, RecognizerCatalog};
use crate::service::{EngineService, ServiceState};

/// Lists the deployment's supported labels (the built-in taxonomy).
async fn list_labels(AuthState(_): AuthState) -> Json<LabelCatalog> {
    Json(LabelCatalog::with_builtins())
}

fn list_labels_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List labels")
        .description(
            "Returns the deployment's built-in label taxonomy: the categories of sensitive data \
             (PII, PHI, PCI, ...) that policies can target.",
        )
        .response::<200, Json<LabelCatalog>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Lists the recognizers the engine has registered, grouped into NER and LLM.
async fn list_recognizers(
    State(engine): State<EngineService>,
    AuthState(_): AuthState,
) -> Json<RecognizerCatalog> {
    let components = engine.engine().components();
    Json(RecognizerCatalog {
        ner: components.ner,
        llm: components.llm,
    })
}

fn list_recognizers_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List recognizers")
        .description(
            "Returns the recognizers the engine has registered, grouped into NER and LLM — each \
             with its name, optional description, and provider. Connection details and \
             credentials are never exposed.",
        )
        .response::<200, Json<RecognizerCatalog>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Returns routes for the deployment catalog.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/catalog/labels/", get_with(list_labels, list_labels_docs))
        .api_route(
            "/catalog/recognizers/",
            get_with(list_recognizers, list_recognizers_docs),
        )
        .with_path_items(|item| item.tag("Catalog"))
}

#[cfg(test)]
mod tests {
    use elide_pipeline::entity::LabelCatalog;

    #[test]
    fn builtin_labels_are_non_empty() {
        let catalog = LabelCatalog::with_builtins();
        assert!(!catalog.is_empty());
    }

    #[test]
    fn builtin_labels_include_email_address() {
        let catalog = LabelCatalog::with_builtins();
        assert!(catalog.iter().any(|label| label.id() == "email_address"));
    }
}
