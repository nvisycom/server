//! All `axum::`[`Router`]s with related `axum::`[`Handler`]s.
//!
//! [`Router`]: axum::routing::Router
//! [`Handler`]: axum::handler::Handler

mod accounts;
mod authentication;
mod constraints;
mod document_files;
mod document_versions;
mod documents;
mod error;
mod monitors;
mod project_invites;
mod project_members;
mod project_websocket;
mod projects;
mod response;
mod utils;

use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use utoipa_axum::router::OpenApiRouter;

pub use crate::extract::ProjectPermission;
pub use crate::handler::error::{Error, ErrorKind, Result};
pub(crate) use crate::handler::response::ErrorResponse;
pub use crate::handler::utils::Pagination;
use crate::middleware::{refresh_token_middleware, require_authentication};
use crate::service::ServiceState;

#[inline]
async fn handler() -> Response {
    ErrorKind::NotFound.into_response()
}

/// Returns an [`OpenApiRouter`] with all routes.
pub fn openapi_routes(state: ServiceState) -> OpenApiRouter<ServiceState> {
    let require_authentication = from_fn_with_state(state.clone(), require_authentication);
    let refresh_token_middleware = from_fn_with_state(state.clone(), refresh_token_middleware);

    OpenApiRouter::new()
        // Private routes.
        .merge(accounts::routes(state.clone()))
        .merge(projects::routes())
        .merge(project_invites::routes())
        .merge(project_members::routes())
        .merge(project_websocket::routes())
        .merge(documents::routes())
        .merge(document_files::routes())
        .merge(document_versions::routes())
        .route_layer(require_authentication)
        .route_layer(refresh_token_middleware)
        // Public routes.
        .merge(authentication::routes())
        .merge(monitors::routes())
        // Fallback.
        .fallback(handler)
}

#[cfg(test)]
mod test {
    use axum_test::TestServer;
    use utoipa_axum::router::OpenApiRouter;

    use crate::handler::openapi_routes;
    use crate::service::{ServiceConfig, ServiceState};

    /// Returns a new [`TestServer`] with the given router.
    pub async fn create_test_server_with_router(
        router: impl Fn(ServiceState) -> OpenApiRouter<ServiceState>,
    ) -> anyhow::Result<TestServer> {
        let config = ServiceConfig::default();
        let state = ServiceState::from_config(&config).await?;
        let router = router(state.clone());
        create_test_server_with_state(router, state).await
    }

    /// Returns a new [`TestServer`] with the given router and state.
    pub async fn create_test_server_with_state(
        router: OpenApiRouter<ServiceState>,
        state: ServiceState,
    ) -> anyhow::Result<TestServer> {
        let app = router.with_state(state);
        let (app, _) = app.split_for_parts();
        let server = TestServer::new(app)?;
        Ok(server)
    }

    /// Returns a new [`TestServer`] with the default router and state.
    pub async fn create_test_server() -> anyhow::Result<TestServer> {
        let config = ServiceConfig::default();
        let state = ServiceState::from_config(&config).await?;
        let router = openapi_routes(state.clone());
        create_test_server_with_state(router, state).await
    }

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let server = create_test_server().await?;
        assert!(server.is_running());
        Ok(())
    }
}
