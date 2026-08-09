//! Deployment catalog: read-only reference data describing what this deployment
//! offers, independent of any workspace.
//!
//! Currently exposes policy templates (a view over the runtime's shipped
//! [`TemplateCatalog`]); recognizers, labels, and redaction operators are
//! expected to join the same `/catalog/*` namespace.
//!
//! Templates are deployment-owned reference data authored in the runtime's
//! `nvisy-template` crate, not persisted rows. Creating a policy from a template
//! copies its `policy` body into a normal, independently-editable workspace
//! policy (see the policy create handler).

use std::sync::LazyLock;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use nvisy_template::{Template, TemplateCatalog};

use crate::extract::{AuthState, Json, Path};
use crate::handler::request::PolicyTemplatePathParams;
use crate::handler::response::{ErrorResponse, PolicyTemplateSummary};
use crate::handler::{Error, Result};
use crate::service::ServiceState;

/// The runtime's built-in template catalog, built once on first access.
static CATALOG: LazyLock<TemplateCatalog> = LazyLock::new(TemplateCatalog::builtin);

/// Returns the latest version of a built-in policy template by id.
pub fn find_template(id: &str) -> Option<Template> {
    CATALOG.latest(id).map(|template| (*template).clone())
}

/// Lists the deployment's policy templates (latest version of each).
async fn list_policy_templates(AuthState(_): AuthState) -> Json<Vec<PolicyTemplateSummary>> {
    // `iter` yields every (id, version); collapse to the latest per id so a
    // listing shows one row per template.
    let mut summaries = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for template in CATALOG.iter().rev() {
        let id = template.id.to_string();
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        summaries.push(PolicyTemplateSummary::from_template(&template));
    }
    summaries.reverse();
    Json(summaries)
}

fn list_policy_templates_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List policy templates")
        .description("Returns the deployment's built-in policy templates (summaries only).")
        .response::<200, Json<Vec<PolicyTemplateSummary>>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Returns a single policy template (latest version), including its policy body.
async fn read_policy_template(
    AuthState(_): AuthState,
    Path(path_params): Path<PolicyTemplatePathParams>,
) -> Result<Json<Template>> {
    let template = find_template(&path_params.template_slug)
        .ok_or_else(|| Error::not_found("policy_template"))?;
    Ok(Json(template))
}

fn read_policy_template_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get policy template")
        .description("Returns a single built-in policy template with its full policy body.")
        .response::<200, Json<Template>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns routes for the deployment catalog.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/catalog/policy-templates/",
            get_with(list_policy_templates, list_policy_templates_docs),
        )
        .api_route(
            "/catalog/policy-templates/{templateSlug}/",
            get_with(read_policy_template, read_policy_template_docs),
        )
        .with_path_items(|item| item.tag("Catalog"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_non_empty() {
        assert!(!CATALOG.is_empty());
    }

    #[test]
    fn find_template_matches_by_id() {
        assert!(find_template("hipaa_safe_harbor").is_some());
        assert!(find_template("does_not_exist").is_none());
    }
}
