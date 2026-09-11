//! Deployment capabilities: read-only reference data describing what this
//! deployment offers, independent of any workspace.
//!
//! Exposes the label taxonomy (the categories of sensitive data policies can
//! target), the recognizers the engine has registered, which connectors can be
//! created, and which sign-in methods are available. All are deployment-owned
//! reference data, not persisted rows: labels come from the runtime's built-in
//! [`LabelCatalog`], recognizers from the configured
//! [`Engine`](elide_pipeline::Engine) lineup, connector availability from the
//! host's configuration, and sign-in methods from the configured auth providers.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use elide_pipeline::entity::LabelCatalog;
use nvisy_file_service::FileService;
use nvisy_file_service::provider::FileServiceProvider;
use nvisy_postgres::types::IdentityProvider;
use schemars::JsonSchema;
use serde::Serialize;

use crate::extract::{AuthState, Json};
use crate::handler::response::RecognizerCatalog;
use crate::response::ErrorResponse;
use crate::service::{EngineService, OidcService, ServiceState};

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
        .response::<401, Json<ErrorResponse>>()
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
        .response::<401, Json<ErrorResponse>>()
}

/// Per-provider availability for the OAuth file services. Each field is `true`
/// only when that provider's OAuth app is configured on the server.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileProviders {
    /// Whether Google Drive can be connected.
    pub google_drive: bool,
    /// Whether Dropbox can be connected.
    pub dropbox: bool,
    /// Whether OneDrive can be connected.
    pub one_drive: bool,
    /// Whether Box can be connected. (`box` is a Rust keyword, hence the field
    /// name; the wire name is `box`.)
    #[serde(rename = "box")]
    pub box_provider: bool,
}

impl FileProviders {
    /// Builds the availability report from the deployment's file service.
    fn from_service(file_service: &FileService) -> Self {
        // Matched exhaustively so adding a `FileServiceProvider` variant fails to compile
        // until it is represented here.
        let is = |provider| file_service.is_configured(provider);
        Self {
            google_drive: is(FileServiceProvider::GoogleDrive),
            dropbox: is(FileServiceProvider::Dropbox),
            one_drive: is(FileServiceProvider::OneDrive),
            box_provider: is(FileServiceProvider::Box),
        }
    }
}

/// Which connector families and providers this deployment can create.
///
/// Lets a client render the connect UI without probing: a file-service provider
/// is only offered when the host has configured its OAuth app, whereas
/// object-store and inference connections carry their own credentials and are
/// always available.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCapabilities {
    /// Availability of each OAuth file-service provider.
    pub file_services: FileProviders,
    /// Whether object-store connections can be created. Currently always `true` —
    /// they carry their own credentials, so nothing gates them server-side — but
    /// clients should read the field rather than assume it.
    pub object_stores: bool,
    /// Whether inference connections can be created. Currently always `true`, for
    /// the same reason as object stores.
    pub inference: bool,
}

/// Reports which connectors this deployment can create.
async fn list_connectors(
    State(file_service): State<FileService>,
    _: AuthState,
) -> Json<ConnectorCapabilities> {
    Json(ConnectorCapabilities {
        file_services: FileProviders::from_service(&file_service),
        object_stores: true,
        inference: true,
    })
}

fn list_connectors_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connectors")
        .description(
            "Returns which connector families and providers this deployment can create: each \
             OAuth file-service provider is available only when its app is configured on the \
             server, while object-store and inference connections carry their own credentials \
             and are always available. Use it to render the connect UI without probing.",
        )
        .response::<200, Json<ConnectorCapabilities>>()
        .response::<401, Json<ErrorResponse>>()
}

/// The sign-in methods this deployment offers.
///
/// Lets a client render the login screen without probing: one ordered list of
/// the identity providers a person can sign in with — `password`, plus exactly
/// the OIDC providers (`google`, `microsoft`, ...) whose apps are configured on
/// the server. This is pre-authentication reference data, so the endpoint is
/// unauthenticated.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthCapabilities {
    /// The available sign-in methods, in the order a client should present them:
    /// `password` first, then each configured OIDC provider.
    pub methods: Vec<IdentityProvider>,
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
/// client reads (labels, recognizers, connectors).
pub fn private_routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/capabilities/labels/",
            get_with(list_labels, list_labels_docs),
        )
        .api_route(
            "/capabilities/recognizers/",
            get_with(list_recognizers, list_recognizers_docs),
        )
        .api_route(
            "/capabilities/connectors/",
            get_with(list_connectors, list_connectors_docs),
        )
        .with_path_items(|item| item.tag("Capabilities"))
}

/// Public capability routes: the sign-in methods a login screen reads before
/// anyone is authenticated, so this route carries no auth requirement.
pub fn public_routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/capabilities/auth/", get_with(list_auth, list_auth_docs))
        .with_path_items(|item| item.tag("Capabilities"))
}

#[cfg(test)]
mod tests {
    use elide_pipeline::entity::LabelCatalog;
    use nvisy_file_service::OAuthApps;
    use nvisy_file_service::oauth::OAuthApp;

    use super::*;

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
}
