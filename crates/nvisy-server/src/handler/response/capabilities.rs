//! Deployment capabilities response types.

use elide_pipeline::RegisteredRecognizer;
use nvisy_core::net::EndpointPolicy;
use nvisy_file_service::FileService;
use nvisy_file_service::provider::FileServiceProvider;
use nvisy_postgres::types::IdentityProvider;
use schemars::JsonSchema;
use serde::Serialize;

use crate::service::DocumentModality;

/// The engine's registered recognizers, grouped by kind.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizerCatalog {
    /// NER (named-entity recognition) recognizers.
    pub ner: Vec<RegisteredRecognizer>,
    /// LLM recognizers.
    pub llm: Vec<RegisteredRecognizer>,
}

/// Per-provider availability for the OAuth file services. Each field is `true`
/// only when that provider's OAuth app is configured on the server.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
// reason: one serialized availability flag per provider; a struct-per-bool would
// not read better and would change the wire shape.
#[allow(clippy::struct_excessive_bools)]
pub struct FileProviders {
    /// Whether Google Drive can be connected.
    pub google_drive: bool,
    /// Whether Dropbox can be connected.
    pub dropbox: bool,
    /// Whether `OneDrive` can be connected.
    pub one_drive: bool,
    /// Whether Box can be connected. (`box` is a Rust keyword, hence the field
    /// name; the wire name is `box`.)
    #[serde(rename = "box")]
    pub box_provider: bool,
}

impl FileProviders {
    /// Builds the availability report from the deployment's file service.
    pub(crate) fn from_service(file_service: &FileService) -> Self {
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

/// What inference providers this deployment can connect to.
///
/// Inference providers carry their own credentials, so creating one is always
/// available. A local-only backend such as Ollama is different: it is reached at
/// a loopback URL, which only a self-hosted deployment's endpoint policy admits —
/// a cloud deployment rejects a loopback endpoint at connection time — so
/// `ollama` reports whether this deployment can use it at all.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InferenceCapabilities {
    /// Whether inference providers can be created. Currently always `true` — they
    /// carry their own credentials, so nothing gates them server-side.
    pub available: bool,
    /// Whether a local Ollama backend can be used, for both the `@assistant`
    /// reviewer and LLM-driven detection. `true` only on a self-hosted
    /// (`permissive`) deployment, whose endpoint policy admits a loopback URL;
    /// a cloud (`strict`) deployment reports `false`.
    pub ollama: bool,
}

impl InferenceCapabilities {
    /// Builds the inference availability report from the deployment's endpoint
    /// policy. Ollama lives at a loopback URL, which only a self-hosted
    /// (permissive) deployment's endpoint policy admits.
    pub(crate) fn from_policy(endpoint_policy: EndpointPolicy) -> Self {
        Self {
            available: true,
            ollama: matches!(endpoint_policy, EndpointPolicy::Permissive),
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
    /// Which inference providers this deployment can connect to.
    pub inference: InferenceCapabilities,
}

/// One file format the engine can process.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupportedFormat {
    /// The engine's format id (e.g. `elide.document.pdf`).
    pub id: String,
    /// The modality the format decodes to.
    pub modality: DocumentModality,
    /// The file extensions that resolve to this format, lowercased with no dot.
    pub extensions: Vec<String>,
    /// The MIME content types that resolve to this format.
    pub content_types: Vec<String>,
}

/// The file formats this deployment can process.
///
/// Uploads and imports of any other format are rejected, so a client builds its
/// file picker's accept list from this rather than a list of its own.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatCapabilities {
    /// The supported formats, in the engine's registration order.
    pub formats: Vec<SupportedFormat>,
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
