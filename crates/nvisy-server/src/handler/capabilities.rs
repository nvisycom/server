//! Deployment capabilities: read-only reference data describing what this
//! deployment offers, independent of any workspace.
//!
//! Exposes the label taxonomy (the categories of sensitive data policies can
//! target), the recognizers the engine has registered, the file formats it can
//! process, which connectors can be created, and which sign-in methods are
//! available. All are deployment-owned reference data, not persisted rows: labels
//! come from the runtime's built-in [`LabelCatalog`], recognizers and formats from
//! the configured [`Engine`], connector availability from the host's
//! configuration, and sign-in methods from the configured auth providers.
//!
//! [`Engine`]: elide_pipeline::Engine

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use elide_pipeline::entity::LabelCatalog;
use nvisy_core::net::EndpointPolicy;
use nvisy_file_service::FileService;
use nvisy_postgres::types::IdentityProvider;

use crate::extract::{AuthState, Json};
use crate::handler::response::{
    AuthCapabilities, ConnectorCapabilities, FileProviders, FormatCapabilities,
    InferenceCapabilities, RecognizerCatalog, SupportedFormat,
};
use crate::service::{EngineService, OidcService, ServiceState, format_modality};

/// Lists the deployment's supported labels (the built-in taxonomy).
async fn list_labels(_: AuthState) -> Json<LabelCatalog> {
    Json(LabelCatalog::with_builtins())
}

fn list_labels_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List labels")
        .description(
            "Returns the deployment's built-in label taxonomy: the categories of sensitive data \
             (PII, PHI, PCI, ...) that policies can target.",
        )
        .response::<200, Json<LabelCatalog>>()
}

/// Lists the recognizers the engine has registered, grouped into NER and LLM.
async fn list_recognizers(
    State(engine): State<EngineService>,
    _: AuthState,
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
}

/// Reports which connectors this deployment can create.
async fn list_connectors(
    State(file_service): State<FileService>,
    State(endpoint_policy): State<EndpointPolicy>,
    _: AuthState,
) -> Json<ConnectorCapabilities> {
    Json(ConnectorCapabilities {
        file_services: FileProviders::from_service(&file_service),
        object_stores: true,
        inference: InferenceCapabilities::from_policy(endpoint_policy),
    })
}

fn list_connectors_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connectors")
        .description(
            "Returns which connector families and providers this deployment can create: each \
             OAuth file-service provider is available only when its app is configured on the \
             server, while object-store and inference connections carry their own credentials \
             and are always available. The inference section also reports whether a local Ollama \
             backend can be used (self-hosted deployments only). Use it to render the connect UI \
             without probing.",
        )
        .response::<200, Json<ConnectorCapabilities>>()
}

/// Reports the file formats the engine can process.
async fn list_formats(
    State(engine): State<EngineService>,
    _: AuthState,
) -> Json<FormatCapabilities> {
    Json(format_capabilities(&engine))
}

/// The user-facing formats in the engine's registry: those with a modality a
/// document can be (metadata-only and test formats are left out).
fn format_capabilities(engine: &EngineService) -> FormatCapabilities {
    let formats = engine
        .engine()
        .formats()
        .iter()
        .filter_map(|format| {
            let id = format.id().as_str();
            Some(SupportedFormat {
                modality: format_modality(id)?,
                id: id.to_owned(),
                extensions: format
                    .extensions()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                content_types: format
                    .content_types()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            })
        })
        .collect();
    FormatCapabilities { formats }
}

fn list_formats_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List supported file formats")
        .description(
            "Returns the file formats this deployment can process, each with its modality, \
             file extensions, and content types. Uploading or importing any other format is \
             rejected, so build a file picker's accept list from this.",
        )
        .response::<200, Json<FormatCapabilities>>()
}

/// Reports the deployment's available sign-in methods.
async fn list_auth(State(oidc): State<OidcService>) -> Json<AuthCapabilities> {
    // Password is always available; the configured OIDC providers follow.
    let mut methods = vec![IdentityProvider::Password];
    methods.extend(oidc.configured_providers());
    Json(AuthCapabilities { methods })
}

fn list_auth_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List sign-in methods")
        .description(
            "Returns the deployment's available sign-in methods: whether password sign-in is \
             enabled and which OIDC providers are configured. Unauthenticated, so a login screen \
             can render the right buttons before anyone signs in.",
        )
        .response::<200, Json<AuthCapabilities>>()
}

/// Authenticated capability routes: the deployment reference data a signed-in
/// client reads (labels, recognizers, formats, connectors).
pub fn private_routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::get_with;

    ApiRouter::new()
        .api_route(
            "/capabilities/labels",
            get_with(list_labels, list_labels_docs),
        )
        .api_route(
            "/capabilities/recognizers",
            get_with(list_recognizers, list_recognizers_docs),
        )
        .api_route(
            "/capabilities/formats",
            get_with(list_formats, list_formats_docs),
        )
        .api_route(
            "/capabilities/connectors",
            get_with(list_connectors, list_connectors_docs),
        )
        .with_path_items(|item| item.tag("Capabilities"))
}

/// Public capability routes: the sign-in methods a login screen reads before
/// anyone is authenticated, so this route carries no auth requirement.
pub fn public_routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::get_with;

    ApiRouter::new()
        .api_route("/capabilities/auth", get_with(list_auth, list_auth_docs))
        .with_path_items(|item| item.tag("Capabilities"))
}

#[cfg(test)]
mod tests {
    use elide_pipeline::entity::LabelCatalog;
    use nvisy_file_service::OAuthApps;
    use nvisy_file_service::oauth::OAuthApp;

    use super::*;
    use crate::service::DocumentModality;

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

    #[test]
    fn ollama_is_available_only_on_a_self_hosted_deployment() {
        // A self-hosted (permissive) deployment admits a loopback Ollama URL; a
        // cloud (strict) one rejects it at connection time, so it reports false.
        assert!(InferenceCapabilities::from_policy(EndpointPolicy::Permissive).ollama);
        assert!(!InferenceCapabilities::from_policy(EndpointPolicy::Strict).ollama);
    }

    #[test]
    fn connectors_report_only_configured_file_service_providers() {
        // Configure only Google Drive; the others must report unavailable.
        let apps = OAuthApps {
            google_drive: Some(OAuthApp {
                client_id: "id".to_owned(),
                client_secret: "secret".to_owned(),
                redirect_uri: "https://example.com/callback".to_owned(),
            }),
            ..Default::default()
        };
        let file_service = FileService::new(apps).unwrap();

        let providers = FileProviders::from_service(&file_service);

        assert!(providers.google_drive);
        assert!(!providers.dropbox);
        assert!(!providers.one_drive);
        assert!(!providers.box_provider);
    }

    #[tokio::test]
    async fn formats_report_what_the_engine_decodes() {
        use crate::service::{EngineConfig, EngineService};

        let engine = EngineService::from_config(EngineConfig { config_path: None })
            .await
            .expect("default engine builds");
        let capabilities = super::format_capabilities(&engine);
        let audio: Vec<&str> = capabilities
            .formats
            .iter()
            .filter(|format| format.modality == DocumentModality::Audio)
            .flat_map(|format| format.extensions.iter().map(String::as_str))
            .collect();
        // WAV is the only audio codec built in: MP3 is opt-in upstream
        // (`codec-mp3`), and there is no Ogg codec.
        assert_eq!(audio, ["wav"]);
        assert!(
            !capabilities
                .formats
                .iter()
                .any(|format| format.extensions.iter().any(|ext| ext == "ogg")),
            "ogg has no codec, so it must not be advertised",
        );
    }
}
