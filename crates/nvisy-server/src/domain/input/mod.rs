//! Domain input types: the payloads domain service methods accept, free of
//! transport concerns (no validation or schema). A type carries serde only where
//! its value is persisted as JSON (a pipeline definition round-trips through the
//! `definition` column), never for request decoding.

mod workspace_connections;
mod workspace_detections;
mod workspace_documents;
mod workspace_invites;
mod workspace_pipelines;
mod workspace_policies;
mod workspace_providers;
mod workspace_webhooks;

pub use workspace_connections::*;
pub use workspace_detections::*;
pub use workspace_documents::*;
pub use workspace_invites::*;
pub use workspace_pipelines::*;
pub use workspace_policies::*;
pub use workspace_providers::*;
pub use workspace_webhooks::*;
