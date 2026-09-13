//! Domain output types: the composite results domain service methods return.
//! Single models and paginated model lists are returned directly, without a
//! wrapper.

mod workspace_connections;
mod workspace_detections;
mod workspace_documents;
mod workspace_invites;
mod workspace_pipelines;
mod workspace_policies;
mod workspace_webhooks;
mod workspaces;

pub use workspace_connections::*;
pub use workspace_detections::*;
pub use workspace_documents::*;
pub use workspace_invites::*;
pub use workspace_pipelines::*;
pub use workspace_policies::*;
pub use workspace_webhooks::*;
pub use workspaces::*;
